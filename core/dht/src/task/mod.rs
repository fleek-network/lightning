mod bootstrap;
mod lookup;

use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
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
    query::{NodeInfo, Response},
    table::{TableKey, TableRequest},
    task::{bootstrap::Bootstrap, lookup::LookupTask},
};

pub async fn start_worker(
    task_tx: Sender<Task>,
    mut rx: Receiver<Task>,
    mut network_event: Receiver<ResponseEvent>,
    table_tx: Sender<TableRequest>,
    socket: Arc<UdpSocket>,
    bootstrap_nodes: Vec<NodeInfo>,
    local_key: NodeNetworkingPublicKey,
) {
    let mut task_set = TaskManager {
        task_queue: DelayQueue::new(),
        inflight: HashMap::new(),
        set: JoinSet::new(),
        local_key,
        table_tx: table_tx.clone(),
        socket,
        bootstrap_task: Bootstrap::new(task_tx, table_tx, local_key.0, bootstrap_nodes.clone()),
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
                todo!()
            }
            response = task_set.set.join_next() => {
                todo!()
            }
        }
    }
}

pub enum Task {
    Bootstrap,
    Lookup {
        target: TableKey,
        refresh_bucket: bool,
        tx: Option<oneshot::Sender<TaskResponse>>,
    },
    Ping {
        target: TableKey,
        rx: oneshot::Sender<()>,
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
    inflight: HashMap<u64, OngoingTask>,
    set: JoinSet<Result<()>>,
    local_key: NodeNetworkingPublicKey,
    table_tx: Sender<TableRequest>,
    socket: Arc<UdpSocket>,
    bootstrap_task: Bootstrap,
}

impl TaskManager {
    fn handle_response(&mut self, event: ResponseEvent) {
        match self.inflight.get(&event.id) {
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
                self.inflight.insert(id, OngoingTask { tx: task_tx });
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
                self.set.spawn(async move {
                    let response = lookup::lookup(lookup).await?;

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
                    Ok(())
                });
            },
            Task::Bootstrap => {
                if self.bootstrap_task.is_idle() || self.bootstrap_task.is_complete() {
                    self.bootstrap_task = self.bootstrap_task.restart();
                }
            },
            Task::Ping { .. } => todo!(),
        }
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
