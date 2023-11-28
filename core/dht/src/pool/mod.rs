mod client;
mod lookup;

use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
pub use client::Client;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodeIndex;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{oneshot, Notify};
use tokio::task::JoinHandle;

use crate::pool::lookup::LookupInterface;
use crate::table::Event;

pub enum Task {
    LookUpValue { hash: u32, respond: ValueRespond },
    LookUpNode { hash: u32, respond: ContactRespond },
    Ping { peer: NodeIndex, timeout: Duration },
}

pub struct NetworkMessage {
    id: u64,
    payload: Bytes,
}

pub struct WorkerPool<C: Collection, L: LookupInterface> {
    /// Performs look-ups.
    looker: L,
    /// Queue for receiving tasks.
    task_queue: Receiver<Task>,
    /// Queue for sending events.
    event_queue: Sender<Event>,
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
                    let value = looker.lookup_value(hash).await;
                    // if the client drops the receiver, there's nothing we can do.
                    let _ = respond.send(value);
                    id
                }));
            },
            Task::LookUpNode { hash, respond } => {
                let looker = self.looker.clone();
                let id: u64 = rand::random();
                self.ongoing_tasks.push(tokio::spawn(async move {
                    let nodes = looker.lookup_contact(hash).await;
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

pub type ValueRespond = oneshot::Sender<lookup::Result<Option<Bytes>>>;
pub type ContactRespond = oneshot::Sender<lookup::Result<Vec<NodeIndex>>>;
