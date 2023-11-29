mod client;
mod lookup;

use std::collections::HashMap;
use std::marker::PhantomData;
use std::net::SocketAddr;
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

use crate::network;
use crate::network::{Message, UdpTransport, UnreliableTransport};
use crate::pool::lookup::LookupInterface;
use crate::table::Event;

pub enum Task {
    LookUpValue { hash: u32, respond: ValueRespond },
    LookUpNode { hash: u32, respond: ContactRespond },
    Ping { peer: NodeIndex, timeout: Duration },
}

pub struct WorkerPool<C, L, T>
where
    C: Collection,
    L: LookupInterface,
    T: UnreliableTransport,
{
    /// Our node index.
    us: NodeIndex,
    /// Performs look-ups.
    looker: L,
    /// Socket to send/recv messages over the network.
    socket: T,
    /// Queue for receiving tasks.
    task_queue: Receiver<Task>,
    /// Queue for sending events.
    event_queue: Sender<Event>,
    /// Set of currently running worker tasks.
    ongoing_tasks: FuturesUnordered<JoinHandle<u32>>,
    /// Logical set of ongoing lookup tasks.
    ongoing: HashMap<u32, ()>,
    /// Maximum pool size.
    max_pool_size: usize,
    /// Shutdown notify.
    shutdown: Arc<Notify>,
    _marker: PhantomData<C>,
}

impl<C, L, T> WorkerPool<C, L, T>
where
    C: Collection,
    L: LookupInterface,
    T: UnreliableTransport,
{
    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                biased;
                _ = self.shutdown.notified() => {
                    tracing::info!("shutting down worker pool");
                    break;
                }
                next = self.task_queue.recv() => {
                    let Some(task) = next else {
                        break;
                    };
                    self.handle_incoming_task(task);
                }
                next = self.socket.recv() => {
                    match next {
                        Ok(message) => {
                            self.handle_message(message);
                        }
                        Err(e) => {
                            tracing::error!("unexpected error from socket: {e:?}");
                            break;
                        }
                    }
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
                let id: u32 = rand::random();
                self.ongoing_tasks.push(tokio::spawn(async move {
                    let value = looker.lookup_value(hash).await;
                    // if the client drops the receiver, there's nothing we can do.
                    let _ = respond.send(value);
                    id
                }));
            },
            Task::LookUpNode { hash, respond } => {
                let looker = self.looker.clone();
                let id: u32 = rand::random();
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

    fn handle_message(&self, message: (NodeIndex, Message)) {
        let (from, message) = message;
        let id = message.id();
        let token = message.token();
        let ty = message.ty();
        match ty {
            network::PING_TYPE => {
                self.pong(id, token, from);
            },
            network::PONG_TYPE => {},
            network::STORE_TYPE => {},
            network::FIND_NODE_TYPE => {},
            network::FIND_NODE_RESPONSE_TYPE => {},
            network::FIND_VALUE_TYPE => {},
            network::FIND_VALUE_RESPONSE_TYPE => {},
            ty => {
                tracing::warn!("invalid message type id received: {ty}")
            },
        }
    }

    pub fn pong(&self, id: u32, token: u32, dst: NodeIndex) {
        if self.ongoing_tasks.len() > self.max_pool_size {
            // Should we buffer it or ignore pings in the case where we're overwhelmed?
        }

        let message = network::pong(id, token, self.us);
        let socket = self.socket.clone();
        self.ongoing_tasks.push(tokio::spawn(async move {
            if let Err(e) = socket.send(message, dst).await {
                tracing::error!("failed to send pong: {e:?}");
            }
            // Since we don't save any state that needs cleanup for
            // this task, it doesn't matter which id we give it.
            rand::random()
        }));
        // We do not update `ongoing` because we don't expect a response for a pong.
    }

    fn cleanup(&mut self, id: u32) {
        self.ongoing.remove(&id);
    }
}

pub type ValueRespond = oneshot::Sender<lookup::Result<Option<Bytes>>>;
pub type ContactRespond = oneshot::Sender<lookup::Result<Vec<NodeIndex>>>;
