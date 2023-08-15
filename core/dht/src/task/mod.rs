pub mod bootstrap;
mod lookup;

use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use anyhow::Error;
use fleek_crypto::NodeNetworkingPublicKey;
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
    task::{bootstrap::Bootstrapper, lookup::LookupTask},
};

pub async fn start_worker(
    mut rx: Receiver<Task>,
    mut network_event: Receiver<ResponseEvent>,
    table_tx: Sender<TableRequest>,
    socket: Arc<UdpSocket>,
    local_key: NodeNetworkingPublicKey,
    bootstrap_task: Bootstrapper,
) {
    let mut task_set = TaskManager {
        task_queue: DelayQueue::new(),
        ongoing: HashMap::new(),
        task_results: JoinSet::new(),
        local_key,
        table_tx: table_tx.clone(),
        socket,
        bootstrap_task,
    };
    loop {
        select! {
            task = rx.recv() => {
                let task = task.expect("all channels to not drop");
                task_set.execute(task);
            }
            Some(task) = std::future::poll_fn(|cx| task_set.task_queue.poll_expired(cx)) => {
                task_set.execute(task.into_inner());
            }
            event = network_event.recv() => {
                let event = event.expect("all channels to not drop");
                task_set.handle_response(event);
            }
            bootstrap_result = task_set.bootstrap_task.advance() => {
                if let Err(e) = bootstrap_result {
                    tracing::error!("unexpected error while bootstrapping: {e:?}");
                    task_set.reset_bootstrapper();
                }
            }
            Some(response) = task_set.task_results.join_next() => {
                let id = match response {
                    Ok(TaskResult::Success(id)) => {
                        id
                    }
                    Ok(TaskResult::Failed { id, error }) => {
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
    Bootstrap,
    Lookup {
        target: TableKey,
        refresh_bucket: bool,
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
    pub source: Option<NodeNetworkingPublicKey>,
}

struct TaskManager {
    task_queue: DelayQueue<Task>,
    ongoing: HashMap<u64, OngoingTask>,
    task_results: JoinSet<TaskResult>,
    local_key: NodeNetworkingPublicKey,
    table_tx: Sender<TableRequest>,
    socket: Arc<UdpSocket>,
    bootstrap_task: Bootstrapper,
}

impl TaskManager {
    fn handle_response(&mut self, event: ResponseEvent) {
        match self.ongoing.get(&event.id) {
            Some(ongoing) => {
                if ongoing.tx.is_closed() {
                    // The task is done so this request is not expected.
                    tracing::warn!("received unexpected response");
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
            } => {
                let (task_tx, task_rx) = mpsc::channel(20);
                self.ongoing.insert(id, OngoingTask { tx: task_tx });
                let lookup = LookupTask::new(
                    id,
                    false,
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
                            return TaskResult::Failed { id, error };
                        },
                    };

                    if refresh_bucket {
                        for node in &response.nodes {
                            let (tx, rx) = oneshot::channel();
                            table_tx
                                .send(TableRequest::AddNode {
                                    node: node.clone(),
                                    tx: Some(tx),
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
                    TaskResult::Success(id)
                });
            },
            Task::Bootstrap => {
                if self.bootstrap_task.is_idle() || self.bootstrap_task.is_complete() {
                    self.bootstrap_task = self.bootstrap_task.restart();
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
                            return TaskResult::Failed {
                                id,
                                error: e.into(),
                            };
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
                            return TaskResult::Failed {
                                id,
                                error: e.into(),
                            };
                        },
                    };

                    if let Err(e) = socket::send_to(&socket, &bytes, address).await {
                        return TaskResult::Failed {
                            id,
                            error: e.into(),
                        };
                    }

                    if tx.send(()).is_err() {
                        tracing::error!("failed to send PING response")
                    }

                    match task_rx.recv().await {
                        None => TaskResult::Failed {
                            id,
                            error: anyhow::anyhow!("sender handle was dropped"),
                        },
                        Some(_) => TaskResult::Success(id),
                    }
                });
            },
        }
    }

    pub fn remove_ongoing(&mut self, id: u64) {
        self.ongoing.remove(&id);
    }

    pub fn reset_bootstrapper(&mut self) {
        self.bootstrap_task = self.bootstrap_task.restart();
    }
}

struct OngoingTask {
    /// Send network event to task.
    tx: Sender<ResponseEvent>,
}

#[derive(Debug)]
pub struct ResponseEvent {
    pub id: u64,
    pub sender_key: NodeNetworkingPublicKey,
    pub response: Response,
}

enum TaskResult {
    Success(u64),
    Failed { id: u64, error: Error },
}
