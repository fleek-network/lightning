use std::collections::HashMap;
use bytes::Bytes;

use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{Notification, RequestHeader, ServiceScope};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_util::sync::CancellationToken;
use crate::logical_pool::LogicalPool;
use crate::overlay::{BroadcastRequest, Message, SendRequest};
use crate::pool::Request;

// Reactor.
pub struct EventReceiver<C: Collection> {
    event_queue: Receiver<Event>,
    handler: LogicalPool<C>,
    notifier: Receiver<Notification>,
    /// Service handles.
    broadcast_service_handles: HashMap<ServiceScope, Sender<(NodeIndex, Bytes)>>,
    /// Service handles.
    send_request_service_handles: HashMap<ServiceScope, Sender<(RequestHeader, Request)>>,
    shutdown: CancellationToken,
}

impl<C> EventReceiver<C>
where
    C: Collection,
{
    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                biased;
                _ = self.shutdown.cancelled() => {
                    todo!()
                }
                _ = self.notifier.recv() => {
                    todo!()
                }
                _ = self.event_queue.recv() => {
                    todo!()
                }
            }
        }
    }
}

pub struct MessageReceived {
    peer: NodeIndex,
    message: Message,
}

pub struct RequestReceived {
    peer: NodeIndex,
    service_scope: ServiceScope,
    request: (RequestHeader, Request),
}

pub enum Event {
    NewConnection,
    ConnectionEnded,
    BroadcastRequest(BroadcastRequest),
    SendRequest(SendRequest),
    MessageReceived(MessageReceived),
    RequestReceived(RequestReceived),
}
