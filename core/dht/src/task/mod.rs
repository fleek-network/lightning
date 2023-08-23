mod lookup;

pub mod bootstrap;

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Error;
use fleek_crypto::NodePublicKey;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot, Notify};
use tokio::task::JoinSet;
use tokio_util::time::DelayQueue;

use crate::bucket::BUCKET_REFRESH_INTERVAL;
use crate::network::{Message, MessageType, Request, Response};
use crate::node::NodeInfo;
use crate::table::{TableKey, TableRequest};
use crate::task::bootstrap::{Bootstrapper, BOOTSTRAP_TASK_ID};
use crate::task::lookup::LookupTask;
use crate::{bucket, socket};

type TaskResult = Result<u64, TaskFailed>;

/// Task worker executes tasks.
#[allow(clippy::too_many_arguments)]
pub async fn start_worker(
    mut task_rx: Receiver<Task>,
    task_tx: Sender<Task>,
    mut network_event_rx: Receiver<ResponseEvent>,
    table_tx: Sender<TableRequest>,
    shutdown_notify: Arc<Notify>,
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
        task_tx,
        socket,
        bootstrapper,
    };
    loop {
        tokio::select! {
            task = task_rx.recv() => {
                let task = match task {
                    Some(task) => task,
                    None => break,
                };
                task_set.execute(task);
            }
            event = network_event_rx.recv() => {
                let event = match event {
                    Some(event) => event,
                    None => break,
                };
                task_set.handle_response(event);
            }
            _ = task_set.advance_tasks() => {}
            _ = shutdown_notify.notified() => {
                tracing::trace!("shutting down task manager worker");
                break
            },
        }
    }
}

/// Kademlia tasks.
#[allow(dead_code)]
pub enum Task {
    /// Task that starts a long running bootstrap task.
    Bootstrap {
        respond: oneshot::Sender<anyhow::Result<()>>,
    },
    /// Lookup task for nodes or values.
    Lookup {
        /// Target key for the task.
        target: TableKey,
        /// This flag will tell the task manager to try to store
        /// the nodes found in this lookup.
        refresh_bucket: bool,
        /// Indicate if lookup task should look for a value.
        is_value: bool,
        respond: Option<oneshot::Sender<TaskResponse>>,
    },
    /// Ping task.
    Ping {
        /// Key of peer.
        target: TableKey,
        /// Address of peer.
        address: SocketAddr,
        respond: oneshot::Sender<()>,
    },
    /// Refresh a bucket.
    RefreshBucket {
        bucket_index: usize,
        delay: Duration,
    },
}

/// Responses from tasks.
#[derive(Default)]
pub struct TaskResponse {
    pub nodes: Vec<NodeInfo>,
    pub value: Option<Vec<u8>>,
    pub rtt: Option<usize>,
    pub source: Option<NodePublicKey>,
}

/// Manages tasks.
struct TaskManager {
    task_queue: DelayQueue<Task>,
    ongoing: HashMap<u64, OngoingTask>,
    task_results: JoinSet<TaskResult>,
    local_key: NodePublicKey,
    table_tx: Sender<TableRequest>,
    task_tx: Sender<Task>,
    socket: Arc<UdpSocket>,
    bootstrapper: Bootstrapper,
}

impl TaskManager {
    /// Routes responses received from the network to running tasks.
    fn handle_response(&mut self, event: ResponseEvent) {
        match self.ongoing.get(&event.id) {
            Some(ongoing) => {
                if ongoing.network_event_tx.is_closed() {
                    // The task is done so this request is not expected.
                    tracing::warn!("received unexpected responseee");
                    return;
                }
                let task_tx = ongoing.network_event_tx.clone();
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

    /// Executes task.
    fn execute(&mut self, task: Task) {
        let id: u64 = rand::random();
        match task {
            Task::Lookup {
                target,
                refresh_bucket,
                respond: tx,
                is_value,
            } => {
                let (task_tx, task_rx) = mpsc::channel(20);
                self.ongoing.insert(
                    id,
                    OngoingTask {
                        network_event_tx: task_tx,
                    },
                );
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
            Task::Bootstrap { respond: tx } => {
                if let Entry::Vacant(entry) = self.ongoing.entry(BOOTSTRAP_TASK_ID) {
                    // Bootstrap task actually doesn't need events from the network.
                    let (event_tx, _) = mpsc::channel(1);
                    entry.insert(OngoingTask {
                        network_event_tx: event_tx,
                    });
                    let bootstrapper = self.bootstrapper.clone();
                    self.task_results.spawn(bootstrapper.start(tx));
                }
            },
            Task::Ping {
                respond: tx,
                address,
                ..
            } => {
                let id = rand::random();
                let socket = self.socket.clone();
                let sender_key = self.local_key;
                let (task_tx, mut task_rx) = mpsc::channel(3);
                self.ongoing.insert(
                    id,
                    OngoingTask {
                        network_event_tx: task_tx,
                    },
                );
                self.task_results.spawn(async move {
                    let payload = match bincode::serialize(&Request::Ping) {
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
            Task::RefreshBucket {
                bucket_index,
                delay,
            } => {
                let random_key = bucket::random_key_in_bucket(bucket_index, &self.local_key.0);
                let (task_tx, task_rx) = mpsc::channel(20);
                self.ongoing.insert(
                    id,
                    OngoingTask {
                        network_event_tx: task_tx,
                    },
                );
                let lookup = LookupTask::new(
                    id,
                    false,
                    self.local_key,
                    TableKey::try_from(random_key.as_slice()).unwrap(),
                    self.table_tx.clone(),
                    task_rx,
                    self.socket.clone(),
                );
                let task_tx = self.task_tx.clone();
                let table_tx = self.table_tx.clone();
                self.task_results.spawn(async move {
                    tokio::time::sleep(delay).await;
                    let response = match lookup::lookup(lookup).await {
                        Ok(response) => response,
                        Err(error) => {
                            return Err(TaskFailed { id, error });
                        },
                    };
                    if !response.nodes.is_empty() {
                        let req = TableRequest::ProposeBucketNodes {
                            bucket_index: 0,
                            nodes: response.nodes,
                        };
                        if let Err(e) = table_tx.send(req).await {
                            tracing::trace!("failed to send new nodes to bucket: {e:?}");
                        }
                    }
                    // Schedule the next bucket refresh. For buckets with higher indices, we will
                    // wait longer to refresh.
                    let task = Task::RefreshBucket {
                        bucket_index,
                        delay: BUCKET_REFRESH_INTERVAL
                            + Duration::from_secs(60 * bucket_index as u64),
                    };
                    if let Err(e) = task_tx.send(task).await {
                        tracing::trace!("failed to send bucket refresh task: {e:?}");
                    }
                    Ok(id)
                });
            },
        }
    }

    /// Advances tasks that are enqueued and ongoing.
    pub async fn advance_tasks(&mut self) {
        tokio::select! {
            Some(task) = std::future::poll_fn(|cx| self.task_queue.poll_expired(cx)) => {
                self.execute(task.into_inner());
            }
            Some(response) = self.task_results.join_next() => {
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
                        return;
                    }
                };
                self.remove_ongoing(id);
            }
            else => {}
        }
    }

    /// Remove ongoing tasks from list.
    pub fn remove_ongoing(&mut self, id: u64) {
        self.ongoing.remove(&id);
    }
}

/// A handle to send response events to running task.
struct OngoingTask {
    /// Send network event to task.
    network_event_tx: Sender<ResponseEvent>,
}

/// Response event trigger by a response received from the network.
#[derive(Debug)]
pub struct ResponseEvent {
    pub id: u64,
    pub token: u64,
    pub sender_key: NodePublicKey,
    pub response: Response,
}

/// Error sent by tasks on failure.
pub struct TaskFailed {
    id: u64,
    error: Error,
}
