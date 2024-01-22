use std::collections::HashMap;
use std::io;

use bytes::{BufMut, Bytes, BytesMut};
use fleek_crypto::NodePublicKey;
use futures::stream::FuturesUnordered;
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
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use x509_parser::nom::AsBytes;

use crate::endpoint::EndpointTask;
use crate::logical_pool::LogicalPool;
use crate::provider::{Request, Response};
use crate::state::{ConnectionInfo, EventReceiverInfo};

/// Events.
pub enum Event {
    NewConnection {
        remote: NodeIndex,
        service_request_sent: bool,
    },
    ConnectionEnded {
        remote: NodeIndex,
    },
    Broadcast {
        service_scope: ServiceScope,
        message: Bytes,
        param: Param,
    },
    SendRequest {
        dst: NodeIndex,
        service_scope: ServiceScope,
        request: Bytes,
        respond: oneshot::Sender<io::Result<Response>>,
    },
    MessageReceived {
        remote: NodeIndex,
        message: Message,
    },
    RequestReceived {
        remote: NodeIndex,
        service_scope: ServiceScope,
        request: (RequestHeader, Request),
    },
    GetStats {
        respond: oneshot::Sender<anyhow::Result<EventReceiverInfo>>,
    },
}

/// Event receiver.
pub struct EventReceiver<C: Collection> {
    /// Queue of events.
    event_queue: Receiver<Event>,
    /// Main handler of events.
    pub(crate) handler: LogicalPool<C>,
    /// Epoch event receiver.
    notifier: Receiver<Notification>,
    /// Writer side of queue for actual pool tasks.
    endpoint_queue: Sender<EndpointTask>,
    /// Service handles.
    broadcast_service_handles: HashMap<ServiceScope, Sender<(NodeIndex, Bytes)>>,
    /// Service handles.
    send_request_service_handles: HashMap<ServiceScope, Sender<(RequestHeader, Request)>>,
    /// Ongoing asynchronous tasks.
    ongoing_asynchronous_tasks: FuturesUnordered<JoinHandle<anyhow::Result<()>>>,
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
        pool_queue: Sender<EndpointTask>,
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
            endpoint_queue: pool_queue,
            broadcast_service_handles: HashMap::new(),
            send_request_service_handles: HashMap::new(),
            ongoing_asynchronous_tasks: FuturesUnordered::new(),
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

    fn setup_state(&mut self) -> anyhow::Result<()> {
        let endpoint_task = self.handler.update_connections();
        self.endpoint_queue
            .try_send(endpoint_task)
            .map_err(Into::into)
    }

    #[inline]
    fn handle_new_epoch(&mut self, epoch_event: Notification) -> anyhow::Result<()> {
        match epoch_event {
            Notification::NewEpoch => {
                let endpoint_task = self.handler.update_connections();
                self.endpoint_queue.try_send(endpoint_task)?;
            },
            _ => {
                unreachable!("we're only registered for new epoch events")
            },
        }
        Ok(())
    }

    #[inline]
    pub(crate) fn handle_event(&mut self, event: Event) -> anyhow::Result<()> {
        match event {
            Event::Broadcast {
                service_scope,
                message,
                param,
            } => {
                let _ = self.handle_outgoing_broadcast(service_scope, message, param);
            },
            Event::SendRequest {
                dst,
                service_scope,
                request,
                respond,
            } => {
                let _ = self.handle_outgoing_request(dst, service_scope, request, respond);
            },
            Event::MessageReceived { remote, message } => {
                let _ = self.handle_incoming_broadcast(remote, message);
            },
            Event::RequestReceived {
                remote,
                service_scope,
                request,
            } => {
                let _ = self.handle_incoming_request(remote, service_scope, request);
            },
            Event::NewConnection {
                remote,
                service_request_sent,
            } => {
                self.handle_new_connection(remote, service_request_sent);
            },
            Event::ConnectionEnded { remote } => {
                self.handle_closed_connection(remote);
            },
            Event::GetStats { respond } => {
                self.get_stats(respond);
            },
        }

        Ok(())
    }

    fn get_stats(&self, respond: oneshot::Sender<anyhow::Result<EventReceiverInfo>>) {
        let endpoint_queue_cap = self.endpoint_queue.capacity();
        let endpoint_queue_max_cap = self.endpoint_queue.max_capacity();
        let endpoint_queue = self.endpoint_queue.clone();
        let mut connection_info = self.handler.connections();
        self.ongoing_asynchronous_tasks
            .push(tokio::spawn(async move {
                let mut connections = HashMap::new();

                let (tx, rx) = oneshot::channel();
                if endpoint_queue
                    .send(EndpointTask::Stats { respond: tx })
                    .await
                    .is_ok()
                {
                    let (mut actual_connections, ongoing_async_tasks) = rx
                        .await
                        .map(|info| (info.connections, info.ongoing_async_tasks))
                        .unwrap_or_default();
                    for (index, info) in connection_info.iter_mut() {
                        let connection_info = ConnectionInfo {
                            from_topology: info.from_topology,
                            pinned: info.pinned,
                            peer: Some(info.node_info.clone()),
                            actual_connections: actual_connections
                                .remove(index)
                                .unwrap_or_default(),
                        };
                        connections.insert(*index, connection_info);
                    }

                    let _ = respond.send(Ok(EventReceiverInfo {
                        connections,
                        endpoint_queue_cap,
                        endpoint_queue_max_cap,
                        ongoing_endpoint_async_tasks: ongoing_async_tasks,
                    }));
                } else {
                    let _ = respond.send(Err(anyhow::anyhow!("failed to get stats")));
                }

                Ok(())
            }));
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
    fn handle_outgoing_broadcast(
        &self,
        service_scope: ServiceScope,
        message: Bytes,
        param: Param,
    ) -> anyhow::Result<()> {
        if let Some(task) = self
            .handler
            .process_outgoing_broadcast(service_scope, message, param)
        {
            self.enqueue_task(task)?;
        }

        Ok(())
    }

    #[inline]
    fn handle_outgoing_request(
        &mut self,
        dst: NodeIndex,
        service_scope: ServiceScope,
        request: Bytes,
        respond: oneshot::Sender<io::Result<Response>>,
    ) -> anyhow::Result<()> {
        if let Some(task) =
            self.handler
                .process_outgoing_request(dst, service_scope, request, respond)
        {
            self.enqueue_task(task)?;
        }

        Ok(())
    }

    #[inline]
    fn handle_incoming_broadcast(&self, remote: NodeIndex, message: Message) -> anyhow::Result<()> {
        if self.handler.process_received_message(&remote) {
            if let Some(sender) = self
                .broadcast_service_handles
                .get(&message.service)
                .cloned()
            {
                sender
                    .try_send((remote, Bytes::from(message.payload)))
                    .unwrap();
            }
        }

        Ok(())
    }

    #[inline]
    fn handle_incoming_request(
        &mut self,
        remote: NodeIndex,
        service_scope: ServiceScope,
        request: (RequestHeader, Request),
    ) -> anyhow::Result<()> {
        if self.handler.process_received_request(remote) {
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
    fn enqueue_task(&self, task: EndpointTask) -> anyhow::Result<()> {
        self.endpoint_queue.try_send(task).map_err(Into::into)
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
        self.setup_state().unwrap();

        loop {
            tokio::select! {
                biased;
                _ = self.shutdown.cancelled() => {
                    break;
                }
                next = self.notifier.recv() => {
                    let Some(event) = next else {
                        println!("notifier was dropped");
                        break;
                    };
                    let _ = self.handle_new_epoch(event);
                }
                next = self.event_queue.recv() => {
                    let Some(event) = next else {
                        break;
                    };
                    let _ = self.handle_event(event);
                }
            }
        }
    }
}

pub enum Param<F = BoxedFilterCallback>
where
    F: Fn(NodeIndex) -> bool,
{
    Filter(F),
    Index(NodeIndex),
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

pub type BoxedFilterCallback = Box<dyn Fn(NodeIndex) -> bool + Send + Sync + 'static>;
