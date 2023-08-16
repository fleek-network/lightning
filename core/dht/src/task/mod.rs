pub mod bootstrap;
mod lookup;

use std::{
    collections::{hash_map::Entry, HashMap},
    net::SocketAddr,
    sync::Arc,
};

use anyhow::Error;
use fleek_crypto::NodePublicKey;
use tokio::{
    net::UdpSocket,
    select,
    sync::{
        mpsc,
        mpsc::{Receiver, Sender},
        oneshot,
    },
    task::JoinSet,
};
use tokio_util::time::DelayQueue;

use crate::{
    query::{Message, MessageType, NodeInfo, Query, Response},
    socket,
    table::{TableKey, TableRequest},
    task::{
        bootstrap::{Bootstrapper, BOOTSTRAP_TASK_ID},
        lookup::LookupTask,
    },
};

pub async fn start_worker(
    mut rx: Receiver<Task>,
    mut network_event: Receiver<ResponseEvent>,
    table_tx: Sender<TableRequest>,
    socket: Arc<UdpSocket>,
    local_key: NodePublicKey,
    bootstrapper: Bootstrapper,
) {
    let mut task_set = TaskManager {
        task_queue: DelayQueue::new(),
        ongoing: HashMap::new(),
        task_results: JoinSet::new(),
        local_key,
        table_tx: table_tx.clone(),
        socket,
        bootstrapper,
    };
    loop {
        select! {
            task = rx.recv() => {
                let task = task.expect("all channels to not drop");
                task_set.execute(task);
            }
            event = network_event.recv() => {
                let event = event.expect("all channels to not drop");
                task_set.handle_response(event);
            }
            Some(task) = std::future::poll_fn(|cx| task_set.task_queue.poll_expired(cx)) => {
                task_set.execute(task.into_inner());
            }
            Some(response) = task_set.task_results.join_next() => {
                let id = match response {
                    Ok(Ok(id)) => {
                        id
                    }
                    Ok(Err(TaskFailed { id, error })) => {
                        tracing::error!("task failed: {error:?}");
                        id
                    }
                    Err(e) => {
                        // JoinError may leave ongoing tasks that
                        // will not get cleaned up.
                        // We should use tokio::Task ids when the
                        // featuer is stable.
                        tracing::error!("internal error: {:?}", e);
                        tracing::warn!("unable to remove task from pending task list");
                        continue;
                    }
                };

                task_set.remove_ongoing(id);
            }
        }
    }
}

#[allow(dead_code)]
pub enum Task {
    Bootstrap {
        tx: oneshot::Sender<anyhow::Result<()>>,
    },
    Lookup {
        target: TableKey,
        refresh_bucket: bool,
        is_value: bool,
        tx: Option<oneshot::Sender<TaskResponse>>,
    },
    Ping {
        target: TableKey,
        address: SocketAddr,
        tx: oneshot::Sender<()>,
    },
}

#[derive(Default)]
pub struct TaskResponse {
    pub nodes: Vec<NodeInfo>,
    pub value: Option<Vec<u8>>,
    pub rtt: Option<usize>,
    pub source: Option<NodePublicKey>,
}

struct TaskManager {
    task_queue: DelayQueue<Task>,
    ongoing: HashMap<u64, OngoingTask>,
    task_results: JoinSet<TaskResult>,
    local_key: NodePublicKey,
    table_tx: Sender<TableRequest>,
    socket: Arc<UdpSocket>,
    bootstrapper: Bootstrapper,
}

impl TaskManager {
    fn handle_response(&mut self, event: ResponseEvent) {
        match self.ongoing.get(&event.id) {
            Some(ongoing) => {
                if ongoing.tx.is_closed() {
                    // The task is done so this request is not expected.
                    tracing::warn!("received unexpected responseee");
                    return;
                }
                let task_tx = ongoing.tx.clone();
                tokio::spawn(async move {
                    if task_tx.send(event).await.is_err() {
                        tracing::error!("tasked dropped ")
                    }
                });
            },
            None => {
                tracing::trace!("event id {:?}", event.id);
                tracing::warn!("received unexpected response");
            },
        }
    }

    fn execute(&mut self, task: Task) {
        let id: u64 = rand::random();
        match task {
            Task::Lookup {
                target,
                refresh_bucket,
                tx,
                is_value,
            } => {
                let (task_tx, task_rx) = mpsc::channel(20);
                self.ongoing.insert(id, OngoingTask { tx: task_tx });
                let lookup = LookupTask::new(
                    id,
                    is_value,
                    self.local_key,
                    target,
                    self.table_tx.clone(),
                    task_rx,
                    self.socket.clone(),
                );
                let table_tx = self.table_tx.clone();
                self.task_results.spawn(async move {
                    let response = match lookup::lookup(lookup).await {
                        Ok(response) => response,
                        Err(error) => {
                            return Err(TaskFailed { id, error });
                        },
                    };

                    if refresh_bucket {
                        for node in &response.nodes {
                            let (tx, rx) = oneshot::channel();
                            table_tx
                                .send(TableRequest::AddNode {
                                    node: node.clone(),
                                    respond: Some(tx),
                                })
                                .await
                                .expect("table worker not to drop channel");
                            if let Err(e) = rx.await.expect("table worker not to drop channel") {
                                tracing::error!("unexpected error while querying table: {e:?}");
                            }
                        }
                    }

                    if let Some(tx) = tx {
                        if tx.send(response).is_err() {
                            tracing::error!("failed to send task response");
                        }
                    }
                    Ok(id)
                });
            },
            Task::Bootstrap { tx } => {
                if let Entry::Vacant(entry) = self.ongoing.entry(BOOTSTRAP_TASK_ID) {
                    // Bootstrap task actually doesn't need events from the network.
                    let (event_tx, _) = mpsc::channel(1);
                    entry.insert(OngoingTask { tx: event_tx });
                    let bootstrapper = self.bootstrapper.clone();
                    self.task_results.spawn(bootstrapper.start(tx));
                }
            },
            Task::Ping { tx, address, .. } => {
                let id = rand::random();
                let socket = self.socket.clone();
                let sender_key = self.local_key;
                let (task_tx, mut task_rx) = mpsc::channel(3);
                self.ongoing.insert(id, OngoingTask { tx: task_tx });
                self.task_results.spawn(async move {
                    let payload = match bincode::serialize(&Query::Ping) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            return Err(TaskFailed {
                                id,
                                error: e.into(),
                            });
                        },
                    };

                    let message = Message {
                        ty: MessageType::Query,
                        id,
                        token: rand::random(),
                        sender_key,
                        payload,
                    };

                    let bytes = match bincode::serialize(&message) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            return Err(TaskFailed {
                                id,
                                error: e.into(),
                            });
                        },
                    };

                    if let Err(e) = socket::send_to(&socket, &bytes, address).await {
                        return Err(TaskFailed {
                            id,
                            error: e.into(),
                        });
                    }

                    if tx.send(()).is_err() {
                        tracing::error!("failed to send PING response")
                    }

                    match task_rx.recv().await {
                        None => Err(TaskFailed {
                            id,
                            error: anyhow::anyhow!("sender handle was dropped"),
                        }),
                        Some(_) => Ok(id),
                    }
                });
            },
        }
    }

    pub fn remove_ongoing(&mut self, id: u64) {
        self.ongoing.remove(&id);
    }
}

struct OngoingTask {
    /// Send network event to task.
    tx: Sender<ResponseEvent>,
}

#[derive(Debug)]
pub struct ResponseEvent {
    pub id: u64,
    pub token: u64,
    pub sender_key: NodePublicKey,
    pub response: Response,
}

pub struct TaskFailed {
    id: u64,
    error: Error,
}

type TaskResult = Result<u64, TaskFailed>;
