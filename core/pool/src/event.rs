use std::collections::HashMap;
use std::io;

use bytes::{BufMut, Bytes, BytesMut};
use fleek_crypto::NodePublicKey;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{
    ApplicationInterface,
    Notification,
    NotifierInterface,
    RequestHeader,
    ServiceScope,
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use x509_parser::nom::AsBytes;

use crate::logical_pool::LogicalPool;
use crate::provider::{Request, Response};
use crate::state::{NodeInfo, Stats};

/// Events.
pub enum Event {
    NewConnection {
        remote: NodeIndex,
        service_request_sent: bool,
    },
    ConnectionEnded(NodeIndex),
    Broadcast(BroadcastRequest),
    SendRequest(SendRequest),
    MessageReceived(MessageReceived),
    RequestReceived(RequestReceived),
}

/// Event receiver.
pub struct EventReceiver<C: Collection> {
    /// Queue of events.
    event_queue: Receiver<Event>,
    /// Main handler of events.
    handler: LogicalPool<C>,
    /// Epoch event receiver.
    notifier: Receiver<Notification>,
    /// Writer side of queue for actual pool tasks.
    pool_queue: Sender<PoolTask>,
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
    pub fn new(
        sync_query: c!(C::ApplicationInterface::SyncExecutor),
        topology: c!(C::TopologyInterface),
        notifier: c!(C::NotifierInterface),
        event_queue: Receiver<Event>,
        pool_queue: Sender<PoolTask>,
        public_key: NodePublicKey,
        shutdown: CancellationToken,
    ) -> Self {
        let (notifier_tx, notifier_rx) = mpsc::channel(16);
        notifier.notify_on_new_epoch(notifier_tx);

        let logical_pool = LogicalPool::<C>::new(sync_query, topology, public_key);

        Self {
            event_queue,
            handler: logical_pool,
            notifier: notifier_rx,
            pool_queue,
            broadcast_service_handles: HashMap::new(),
            send_request_service_handles: HashMap::new(),
            shutdown,
        }
    }

    pub fn register_broadcast_service(
        &mut self,
        service: ServiceScope,
    ) -> Receiver<(NodeIndex, Bytes)> {
        let (tx, rx) = mpsc::channel(96);
        self.broadcast_service_handles.insert(service, tx);
        rx
    }

    pub fn register_requester_service(
        &mut self,
        service: ServiceScope,
    ) -> Receiver<(RequestHeader, Request)> {
        let (tx, rx) = mpsc::channel(96);
        self.send_request_service_handles.insert(service, tx);
        rx
    }

    async fn setup_state(&mut self) -> anyhow::Result<()> {
        let (tx, rx) = oneshot::channel();
        let pool_task = self.handler.update_connections(Some(tx));
        self.pool_queue.send(pool_task).await?;
        rx.await.map_err(Into::into)
    }

    #[inline]
    fn handle_new_epoch(&mut self, epoch_event: Notification) -> anyhow::Result<()> {
        match epoch_event {
            Notification::NewEpoch => {
                let pool_task = self.handler.update_connections(None);
                self.pool_queue.try_send(pool_task)?;
            },
            Notification::BeforeEpochChange => {
                unreachable!("we're only registered for new epoch events")
            },
        }
        Ok(())
    }

    #[inline]
    fn handle_event(&mut self, event: Event) -> anyhow::Result<()> {
        match event {
            Event::Broadcast(request) => {
                let _ = self.handle_outgoing_broadcast(request);
            },
            Event::SendRequest(request) => {
                let _ = self.handle_outgoing_request(request);
            },
            Event::MessageReceived(message) => {
                let _ = self.handle_incoming_broadcast(message);
            },
            Event::RequestReceived(request) => {
                let _ = self.handle_incoming_request(request);
            },
            Event::NewConnection {
                remote,
                service_request_sent,
            } => {
                self.handle_new_connection(remote, service_request_sent);
            },
            Event::ConnectionEnded(index) => {
                self.handle_closed_connection(index);
            },
        }

        Ok(())
    }

    #[inline]
    fn handle_new_connection(&mut self, peer: NodeIndex, service_request_sent: bool) {
        self.handler
            .handle_new_connection(peer, service_request_sent);
    }

    #[inline]
    fn handle_closed_connection(&mut self, peer: NodeIndex) {
        self.handler.clean(peer);
    }

    #[inline]
    fn handle_outgoing_broadcast(&self, request: BroadcastRequest) -> anyhow::Result<()> {
        if let Some(task) = self.handler.process_outgoing_broadcast(request) {
            self.enqueue_pool_task(task)?;
        }

        Ok(())
    }

    #[inline]
    fn handle_outgoing_request(&mut self, request: SendRequest) -> anyhow::Result<()> {
        if let Some(task) = self.handler.process_outgoing_request(request) {
            self.enqueue_pool_task(task)?;
        }

        Ok(())
    }

    #[inline]
    fn handle_incoming_broadcast(&self, message: MessageReceived) -> anyhow::Result<()> {
        let MessageReceived {
            peer,
            message: Message { service, payload },
        } = message;

        if self.handler.process_received_message(&peer) {
            if let Some(sender) = self.broadcast_service_handles.get(&service).cloned() {
                sender.try_send((peer, Bytes::from(payload))).unwrap();
            }
        }

        Ok(())
    }

    #[inline]
    fn handle_incoming_request(&mut self, request: RequestReceived) -> anyhow::Result<()> {
        let RequestReceived {
            peer,
            service_scope,
            request,
        } = request;

        if self.handler.process_received_request(peer) {
            if let Some(sender) = self
                .send_request_service_handles
                .get(&service_scope)
                .cloned()
            {
                sender.try_send(request)?;
            }
        }

        Ok(())
    }

    #[inline]
    fn enqueue_pool_task(&self, task: PoolTask) -> anyhow::Result<()> {
        self.pool_queue.try_send(task).map_err(Into::into)
    }

    pub fn clear_state(&mut self) {
        self.handler.clear_state();
    }

    pub fn spawn(mut self) -> JoinHandle<Self> {
        tokio::spawn(async move {
            self.run().await;
            self
        })
    }

    pub async fn run(&mut self) {
        // If there is an error while setting up the state,
        // there is nothing else to do and
        // we should not allow start to proceed.
        let _ = self.setup_state().await.unwrap();

        loop {
            tokio::select! {
                biased;
                _ = self.shutdown.cancelled() => {
                    break;
                }
                next = self.notifier.recv() => {
                    if let Some(event) = next {
                        let _ = self.handle_new_epoch(event);
                    }  else {
                        tracing::info!("notifier was dropped");
                    }
                }
                next = self.event_queue.recv() => {
                    if let Some(event) = next {
                        let _ = self.handle_event(event);
                    }
                }
            }
        }
    }
}

pub struct MessageReceived {
    pub peer: NodeIndex,
    pub message: Message,
}

pub struct RequestReceived {
    pub peer: NodeIndex,
    pub service_scope: ServiceScope,
    pub request: (RequestHeader, Request),
}

/// Requests that will be performed on a connection.
#[allow(dead_code)]
pub enum PoolTask {
    SendMessage {
        peers: Vec<ConnectionInfo>,
        message: Message,
    },
    SendRequest {
        dst: NodeInfo,
        service: ServiceScope,
        request: Bytes,
        respond: oneshot::Sender<io::Result<Response>>,
    },
    Update {
        // Nodes that are in our overlay.
        keep: HashMap<NodeIndex, ConnectionInfo>,
        drop: Vec<NodeIndex>,
        respond: Option<oneshot::Sender<()>>,
    },
    Stats {
        respond: oneshot::Sender<Stats>,
    },
}

pub struct BroadcastRequest<F = BoxedFilterCallback>
where
    F: Fn(NodeIndex) -> bool,
{
    pub service_scope: ServiceScope,
    pub message: Bytes,
    pub param: Param<F>,
}

pub enum Param<F>
where
    F: Fn(NodeIndex) -> bool,
{
    Filter(F),
    Index(NodeIndex),
}

pub struct SendRequest {
    pub peer: NodeIndex,
    pub service_scope: ServiceScope,
    pub request: Bytes,
    pub respond: oneshot::Sender<io::Result<Response>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConnectionInfo {
    /// Pinned connections should not be dropped
    /// on topology changes.
    pub pinned: bool,
    /// This connection was initiated on a topology event.
    pub from_topology: bool,
    /// The info of the peer.
    pub node_info: NodeInfo,
    /// This field is used during topology updates.
    /// It tells the pool whether it should connect
    /// to the peer or wait for the peer to connect.
    pub connect: bool,
}

#[derive(Clone, Debug)]
pub struct Message {
    pub service: ServiceScope,
    pub payload: Vec<u8>,
}

impl TryFrom<BytesMut> for Message {
    type Error = anyhow::Error;

    fn try_from(value: BytesMut) -> anyhow::Result<Self> {
        let bytes = value.as_bytes();
        if bytes.is_empty() {
            return Err(anyhow::anyhow!("Cannot convert empty bytes into a message"));
        }
        let service = ServiceScope::try_from(bytes[0])?;
        let payload = bytes[1..bytes.len()].to_vec();
        Ok(Self { service, payload })
    }
}

impl From<Message> for Bytes {
    fn from(value: Message) -> Self {
        let mut buf = BytesMut::with_capacity(value.payload.len() + 1);
        buf.put_u8(value.service as u8);
        buf.put_slice(&value.payload);
        buf.into()
    }
}

//
pub type BoxedFilterCallback = Box<dyn Fn(NodeIndex) -> bool + Send + Sync + 'static>;
