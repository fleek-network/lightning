mod lookup;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodeIndex;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use crate::pool::lookup::{LookupInterface, ProviderRespond, ValueRespond};

pub enum Task {
    LookUpValue { hash: u32, respond: ValueRespond },
    LookUpNode { hash: u32, respond: ProviderRespond },
    Ping { peer: NodeIndex, timeout: Duration },
}

pub struct NetworkMessage {
    id: u64,
    payload: Bytes,
}

pub enum Event {
    Pong { peer: NodeIndex },
    Timeout { peer: NodeIndex },
}

// 1. Manage tasks from user.
// 2. Read messages from incoming-queue which are messages received from the network.
//      - These include tasks like FIND_NODE or STORE_VALUE.
//      - Some include RPC responses that must be passed on to ongoing tasks.
// 3. (Needs thinking because this might be able to be done by Table itself) Schedule routine tasks
//    related to pruning table.
pub struct WorkerPool<C: Collection, L: LookupInterface> {
    /// Performs look-ups.
    looker: L,
    /// Queue for receiving tasks.
    task_queue: Receiver<Task>,
    /// Queue for sending ping events.
    ping_queue_tx: Sender<Event>,
    /// Set of currently running worker tasks.
    ongoing_tasks: FuturesUnordered<JoinHandle<u64>>,
    /// Logical set of ongoing lookup tasks.
    ongoing: HashMap<u64, ()>,
    /// Maximum pool size.
    max_pool_size: usize,
    /// Shutdown notify.
    shutdown: Arc<Notify>,
    _marker: PhantomData<C>,
}

impl<C: Collection, L: LookupInterface> WorkerPool<C, L> {
    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                biased;
                _ = self.shutdown.notified() => {
                    tracing::info!("shutting down worker pool");
                    break;
                }
                incoming_task = self.task_queue.recv() => {
                    let Some(task) = incoming_task else {
                        break;
                    };
                    self.handle_incoming_task(task);
                }
                Some(task_result) = self.ongoing_tasks.next() => {
                    match task_result {
                        Ok(id) => self.cleanup(id),
                        Err(e) => {
                            tracing::warn!("failed to clean up task: {e:?}");
                        }
                    };
                }
            }
        }
    }

    /// Try to run the task on a worker.
    fn handle_incoming_task(&mut self, task: Task) {
        // Validate.
        if self.ongoing_tasks.len() > self.max_pool_size {
            // Should we add a buffer?
        }

        match task {
            Task::LookUpValue { hash, respond } => {
                let looker = self.looker.clone();
                let id: u64 = rand::random();
                self.ongoing_tasks.push(tokio::spawn(async move {
                    let value = looker.find_value(hash).await;
                    // if the client drops the receiver, there's nothing we can do.
                    let _ = respond.send(value);
                    id
                }));
            },
            Task::LookUpNode { hash, respond } => {
                let looker = self.looker.clone();
                let id: u64 = rand::random();
                self.ongoing_tasks.push(tokio::spawn(async move {
                    let nodes = looker.find_node(hash).await;
                    // if the client drops the receiver, there's nothing we can do.
                    let _ = respond.send(nodes);
                    id
                }));
            },
            Task::Ping { .. } => {
                self.ongoing_tasks.push(tokio::spawn(async move { 0 }));
            },
        }
    }

    fn cleanup(&mut self, id: u64) {
        debug_assert!(self.ongoing.remove(&id).is_some());
    }
}
