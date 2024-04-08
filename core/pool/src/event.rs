use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::sync::Arc;
use std::time::Duration;

use bytes::{BufMut, Bytes, BytesMut};
use fleek_crypto::NodePublicKey;
use futures::stream::FuturesUnordered;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{ApplicationInterface, RequestHeader, ServiceScope, ShutdownWaiter};
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tracing::info;
use x509_parser::nom::AsBytes;

use crate::endpoint::EndpointTask;
use crate::logical_pool::LogicalPool;
use crate::provider::{Request, Response};
use crate::state::{ConnectionInfo, DialInfo, EventReceiverInfo};

/// If a connection ended and the duration was shorter than `CONN_DURATION_THRESHOLD`,
/// we assume that something is wrong, and wait before re-trying the connection.
const CONN_DURATION_THRESHOLD: Duration = Duration::from_secs(30);

/// The minimum amount of time that we wait before re-trying a failed connection.
const CONN_MIN_RETRY_DELAY: Duration = Duration::from_secs(10);

/// The maximum amount of time that we wait before re-trying a failed connection.
const CONN_MAX_RETRY_DELAY: Duration = Duration::from_secs(1800); // 30 minutes

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
    /// Topology update receiver.
    topology_rx: watch::Receiver<Arc<Vec<Vec<NodePublicKey>>>>,
    /// Writer side of queue for actual pool tasks.
    endpoint_queue: Sender<EndpointTask>,
    /// Service handles.
    broadcast_service_handles: HashMap<ServiceScope, Sender<(NodeIndex, Bytes)>>,
    /// Service handles.
    send_request_service_handles: HashMap<ServiceScope, Sender<(RequestHeader, Request)>>,
    /// Ongoing asynchronous tasks.
    ongoing_async_tasks: FuturesUnordered<JoinHandle<anyhow::Result<()>>>,
    /// Information about attempted connection dials.
    dial_info: Arc<scc::HashMap<NodeIndex, DialInfo>>,
}

impl<C> EventReceiver<C>
where
    C: Collection,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        sync_query: c!(C::ApplicationInterface::SyncExecutor),
        topology_rx: watch::Receiver<Arc<Vec<Vec<NodePublicKey>>>>,
        event_queue: Receiver<Event>,
        pool_queue: Sender<EndpointTask>,
        public_key: NodePublicKey,
        dial_info: Arc<scc::HashMap<NodeIndex, DialInfo>>,
    ) -> Self {
        let logical_pool = LogicalPool::<C>::new(sync_query.clone(), public_key);

        Self {
            event_queue,
            handler: logical_pool,
            topology_rx,
            endpoint_queue: pool_queue,
            broadcast_service_handles: HashMap::new(),
            send_request_service_handles: HashMap::new(),
            ongoing_async_tasks: FuturesUnordered::new(),
            dial_info,
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

    fn setup_state(&mut self, connections: Arc<Vec<Vec<NodePublicKey>>>) {
        let endpoint_task = self.handler.update_connections(connections);
        self.enqueue_endpoint_task(endpoint_task);
    }

    #[inline]
    fn enqueue_received_message(
        &self,
        sender: Sender<(NodeIndex, Bytes)>,
        message: (NodeIndex, Bytes),
    ) {
        self.spawn_task(async move {
            let _ = sender.send(message).await;
            Ok(())
        });
    }

    #[inline]
    fn enqueue_received_request(
        &self,
        sender: Sender<(RequestHeader, Request)>,
        request: (RequestHeader, Request),
    ) {
        self.spawn_task(async move {
            let _ = sender.send(request).await;
            Ok(())
        });
    }

    #[inline]
    fn enqueue_endpoint_task(&self, task: EndpointTask) {
        let sender = self.endpoint_queue.clone();
        self.spawn_task(async move {
            let _ = sender.send(task).await;
            Ok(())
        });
    }

    #[inline]
    fn spawn_task<F>(&self, fut: F)
    where
        F: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        // Todo: add a limit.
        self.ongoing_async_tasks.push(tokio::spawn(fut));
    }

    #[inline]
    fn handle_new_topology(&mut self) {
        // We clone the reference to the underlying value because
        // outstanding borrows hold a read lock which can cause
        // the sender to block.
        let conns = self.topology_rx.borrow_and_update().clone();
        let endpoint_task = self.handler.update_connections(conns);
        self.enqueue_endpoint_task(endpoint_task);
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
                self.handle_incoming_broadcast(remote, message);
            },
            Event::RequestReceived {
                remote,
                service_scope,
                request,
            } => {
                self.handle_incoming_request(remote, service_scope, request);
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
        self.ongoing_async_tasks.push(tokio::spawn(async move {
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
                        actual_connections: actual_connections.remove(index).unwrap_or_default(),
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
        if let Some(info) = self.handler.pool.get(&peer) {
            if info.from_topology {
                // A connection to a peer in our topology was ended or failed.
                // We want to retry the connection.

                let mut healthy_conn = true;
                // Calculate the delay before re-trying the connection based on past failed
                // attempts and previous connection duration.
                let mut delay = Some(CONN_MIN_RETRY_DELAY);
                if let Some(dial_info) = self.dial_info.get(&peer) {
                    let dial_info = dial_info.get();
                    if dial_info.last_try.elapsed() < CONN_DURATION_THRESHOLD {
                        // connection was not healthy
                        let max_n = ((CONN_MAX_RETRY_DELAY.as_secs_f64())
                            / (CONN_MIN_RETRY_DELAY.as_secs_f64()))
                        .log2()
                        .ceil();
                        delay = Some(
                            (CONN_MIN_RETRY_DELAY
                                * 2_u32.pow(dial_info.num_tries.max(max_n as u32)))
                            .min(CONN_MAX_RETRY_DELAY),
                        );
                        healthy_conn = false;
                    }
                }

                if healthy_conn {
                    // connection was healthy, we can reset the counter
                    self.reset_dial_info(peer);
                }

                self.enqueue_endpoint_task(EndpointTask::Add {
                    node: peer,
                    info: info.clone(),
                    delay,
                });
            }
        }
        self.handler.clean(peer);
    }

    #[inline]
    fn reset_dial_info(&self, node: NodeIndex) {
        // Warning: this method should not be called while holding a reference into the hashmap.
        if self.dial_info.contains(&node) {
            self.dial_info.update(&node, |_, info| DialInfo {
                num_tries: 0,
                last_try: info.last_try,
            });
        }
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
            self.enqueue_endpoint_task(task);
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
            self.enqueue_endpoint_task(task);
        }

        Ok(())
    }

    #[inline]
    fn handle_incoming_broadcast(&self, remote: NodeIndex, message: Message) {
        if self.handler.process_received_message(&remote) {
            if let Some(sender) = self
                .broadcast_service_handles
                .get(&message.service)
                .cloned()
            {
                self.enqueue_received_message(sender, (remote, message.payload.into()));
            }
        }
    }

    #[inline]
    fn handle_incoming_request(
        &mut self,
        remote: NodeIndex,
        service_scope: ServiceScope,
        request: (RequestHeader, Request),
    ) {
        if self.handler.process_received_request(remote) {
            if let Some(sender) = self
                .send_request_service_handles
                .get(&service_scope)
                .cloned()
            {
                self.enqueue_received_request(sender, request);
            }
        }
    }

    pub fn spawn(mut self, shutdown: ShutdownWaiter) {
        tokio::spawn(async move {
            shutdown.run_until_shutdown(self.run()).await;

            self.handler.clear_state();
            for task in self.ongoing_async_tasks.iter() {
                task.abort();
            }
            self.ongoing_async_tasks.clear();
            while !matches!(self.event_queue.try_recv(), Err(TryRecvError::Empty)) {}
        });
    }

    pub async fn run(&mut self) {
        // If there is an error while setting up the state,
        // there is nothing else to do and
        // we should not allow start to proceed.
        let conns = self.topology_rx.borrow_and_update().clone();
        self.setup_state(conns);

        loop {
            tokio::select! {
                next = self.topology_rx.changed() => {
                    if let Err(e) = next {
                        info!("topology was dropped: {e:?}");
                        break;
                    }
                    self.handle_new_topology();
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
