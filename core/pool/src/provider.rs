use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::Poll;

use anyhow::{Context, Result};
use bytes::Bytes;
use futures::{SinkExt, Stream};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{NodeIndex, RejectReason};
use lightning_interfaces::{RequestHeader, ServiceScope};
use lightning_types::Param;
use ready::ReadyWaiter;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot};
use types::PeerFilter;

use crate::config::Config;
use crate::endpoint::{Endpoint, EndpointTask};
use crate::event::{Event, EventReceiver};
use crate::muxer::quinn::QuinnMuxer;
use crate::muxer::{BoxedChannel, MuxerInterface};
use crate::ready::{PoolReadyState, PoolReadyWaiter};
use crate::{http, muxer, tls};

pub struct PoolProvider<C, M = QuinnMuxer>
where
    C: Collection,
    M: MuxerInterface,
{
    #[allow(clippy::type_complexity)]
    state: Mutex<Option<(Endpoint<C, M>, EventReceiver<C>)>>,
    event_queue: Sender<Event>,
    endpoint_task_queue: Sender<EndpointTask>,
    config: Config,
    ready: PoolReadyWaiter,
}

impl<C: Collection> PoolProvider<C, QuinnMuxer> {
    fn init(
        config: &C::ConfigProviderInterface,
        keystore: &C::KeystoreInterface,
        topology: &C::TopologyInterface,
        sync_query: fdi::Cloned<c!(C::ApplicationInterface::SyncExecutor)>,
    ) -> Result<Self> {
        let config: Config = config.get::<Self>();
        let sk = keystore.get_ed25519_sk();
        let public_key = keystore.get_ed25519_pk();

        let mut transport_config = quinn::TransportConfig::default();
        transport_config.max_idle_timeout(Some(config.max_idle_timeout.try_into()?));
        let tls_config = tls::make_server_config(&sk).expect("Secret key to be valid");
        let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(tls_config));
        server_config.transport_config(Arc::new(transport_config));
        let muxer_config = muxer::quinn::Config {
            server_config,
            address: config.address,
            sk,
            max_idle_timeout: config.max_idle_timeout,
        };

        let dial_info = Arc::new(scc::HashMap::default());
        let (endpoint_task_tx, endpoint_task_rx) = mpsc::channel(1024);
        let (event_tx, event_rx) = mpsc::channel(1024);
        let receiver = EventReceiver::<C>::new(
            sync_query.clone(),
            topology.get_receiver(),
            event_rx,
            endpoint_task_tx.clone(),
            public_key,
            dial_info.clone(),
        );
        let ready = PoolReadyWaiter::new();
        let endpoint = Endpoint::<C, QuinnMuxer>::new(
            sync_query.clone(),
            endpoint_task_rx,
            event_tx.clone(),
            dial_info,
            muxer_config,
        );

        Ok(Self {
            state: Some((endpoint, receiver)).into(),
            event_queue: event_tx,
            endpoint_task_queue: endpoint_task_tx,
            config,
            ready,
        })
    }

    async fn start(this: fdi::Ref<Self>, fdi::Cloned(shutdown): fdi::Cloned<ShutdownWaiter>) {
        let (endpoint, receiver) = this
            .state
            .lock()
            .unwrap()
            .take()
            .expect("start can only be called once");
        let event_tx = this.event_queue.clone();
        let endpoint_task_tx = this.endpoint_task_queue.clone();
        let config = this.config.clone();
        let ready = this.ready.clone();
        drop(this);

        if let Some(http_server_address) = config.http {
            let shutdown = shutdown.clone();
            spawn!(
                async move {
                    http::spawn_http_server(
                        http_server_address,
                        shutdown.clone(),
                        event_tx,
                        endpoint_task_tx,
                    )
                    .await
                },
                "POOL: spawn http server"
            );
        }

        endpoint.spawn(ready, shutdown.clone());
        receiver.spawn(shutdown.clone());
    }

    fn register_broadcast_service(
        &self,
        service: ServiceScope,
    ) -> (Sender<Event>, Receiver<(NodeIndex, Bytes)>) {
        if let Some((_, receiver)) = &mut *self.state.lock().unwrap() {
            (
                self.event_queue.clone(),
                receiver.register_broadcast_service(service),
            )
        } else {
            panic!("failed to start: endpoint is already running");
        }
    }

    fn register_requester_service(
        &self,
        service: ServiceScope,
    ) -> (Sender<Event>, Receiver<(RequestHeader, Request)>) {
        if let Some((_, receiver)) = &mut *self.state.lock().unwrap() {
            (
                self.event_queue.clone(),
                receiver.register_requester_service(service),
            )
        } else {
            panic!("failed to start: endpoint is already running");
        }
    }
}

impl<C> ConfigConsumer for PoolProvider<C, QuinnMuxer>
where
    C: Collection,
{
    const KEY: &'static str = "pool";
    type Config = Config;
}

impl<C: Collection> BuildGraph for PoolProvider<C, QuinnMuxer> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new()
            .with(Self::init.with_event_handler("start", Self::start.wrap_with_spawn_named("POOL")))
    }
}

// Todo: An improvement would be to pass a `Muxer` in `init`.
// See comments in `MuxerInterface`.
impl<C: Collection> PoolInterface<C> for PoolProvider<C, QuinnMuxer> {
    type EventHandler = EventHandler;
    type Requester = Requester;
    type Responder = Responder;
    type ReadyState = PoolReadyState;

    fn open_event(&self, service: ServiceScope) -> Self::EventHandler {
        let (tx, rx) = self.register_broadcast_service(service);
        EventHandler {
            event_rx: rx,
            request_tx: tx,
            service_scope: service,
        }
    }

    fn open_req_res(&self, service: ServiceScope) -> (Self::Requester, Self::Responder) {
        let (tx, rx) = self.register_requester_service(service);
        (
            Requester {
                request_tx: tx,
                service_scope: service,
            },
            Responder { inner: rx },
        )
    }

    /// Wait for the pool to be ready and return the ready state.
    async fn wait_for_ready(&self) -> io::Result<Self::ReadyState> {
        Ok(self.ready.wait().await)
    }

    /// Returns the local address the pool is listening on.
    ///
    /// This is useful for other services to connect to the pool, especially when
    /// the pool is not bound to a specific address (i.e. `0.0.0.0:0` or `[::]:0`),
    /// in which case the OS will assign a random available port.
    fn listen_address(&self) -> Option<SocketAddr> {
        if !self.ready.is_ready() {
            return None;
        }
        self.ready.state().and_then(|state| state.listen_address)
    }

    /// Returns the list of connected peers.
    async fn connected_peers(&self) -> Result<Vec<NodeIndex>> {
        let (tx, rx) = oneshot::channel();

        self.endpoint_task_queue
            .send(EndpointTask::Stats { respond: tx })
            .await
            .context("failed to send stats request to endpoint")?;
        let stats = rx.await.context("failed to get stats from endpoint")?;

        Ok(stats.connections.keys().cloned().collect())
    }
}

pub struct EventHandler {
    request_tx: Sender<Event>,
    event_rx: Receiver<(NodeIndex, Bytes)>,
    service_scope: ServiceScope,
}

impl EventHandlerInterface for EventHandler {
    fn send_to_all<F: Fn(NodeIndex) -> bool + Send + Sync + 'static>(
        &self,
        message: Bytes,
        filter: F,
    ) {
        let request_tx = self.request_tx.clone();
        let service_scope = self.service_scope;
        spawn!(
            async move {
                if request_tx
                    .send(Event::Broadcast {
                        service_scope,
                        message,
                        peer_filter: PeerFilter {
                            param: Param::Filter(Box::new(filter)),
                            by_topology: false,
                        },
                    })
                    .await
                    .is_err()
                {
                    tracing::error!("failed to send to broadcast service");
                }
            },
            "POOL: send to all"
        );
    }

    fn send_to_one(&self, index: NodeIndex, message: Bytes) {
        let request_tx = self.request_tx.clone();
        let service_scope = self.service_scope;
        spawn!(
            async move {
                if request_tx
                    .send(Event::Broadcast {
                        service_scope,
                        message,
                        peer_filter: PeerFilter {
                            param: Param::Index(index),
                            by_topology: false,
                        },
                    })
                    .await
                    .is_err()
                {
                    tracing::error!("failed to send to broadcast service");
                }
            },
            "POOL: send to one"
        );
    }

    // This method is cancel-safe.
    async fn receive(&mut self) -> Option<(NodeIndex, Bytes)> {
        self.event_rx.recv().await
    }
}

pub struct Requester {
    request_tx: Sender<Event>,
    service_scope: ServiceScope,
}

impl Clone for Requester {
    fn clone(&self) -> Self {
        Self {
            request_tx: self.request_tx.clone(),
            service_scope: self.service_scope,
        }
    }
}

impl RequesterInterface for Requester {
    type Response = Response;

    async fn request(&self, peer: NodeIndex, request: Bytes) -> io::Result<Self::Response> {
        let (respond_tx, respond_rx) = oneshot::channel();
        // Request a stream.
        self.request_tx
            .send(Event::SendRequest {
                dst: peer,
                service_scope: self.service_scope,
                request,
                respond: respond_tx,
            })
            .await
            .map_err(|_| io::ErrorKind::BrokenPipe)?;

        let response = respond_rx.await.map_err(|_| io::ErrorKind::BrokenPipe)??;

        Ok(response)
    }
}

pub struct Response {
    status: Status,
    channel: BoxedChannel,
}

impl Response {
    pub(crate) fn new(status: Status, channel: BoxedChannel) -> Self {
        Self { status, channel }
    }
}

impl ResponseInterface for Response {
    type Body = Body;
    fn status_code(&self) -> Result<(), RejectReason> {
        match &self.status {
            Status::Ok => Ok(()),
            Status::Failed(reason) => Err(*reason),
        }
    }

    fn body(self) -> Self::Body {
        Body {
            channel: self.channel,
        }
    }
}

pub struct Body {
    channel: BoxedChannel,
}

impl Stream for Body {
    type Item = io::Result<Bytes>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.channel)
            .poll_next(cx)
            .map_err(Into::into)
    }
}

pub struct Responder {
    inner: Receiver<(RequestHeader, Request)>,
}

impl ResponderInterface for Responder {
    type Request = Request;

    // Note: this method is cancel-safe.
    async fn get_next_request(&mut self) -> io::Result<(RequestHeader, Self::Request)> {
        self.inner
            .recv()
            .await
            .ok_or_else(|| io::Error::from(io::ErrorKind::BrokenPipe))
    }
}

pub struct Request {
    // To properly clean up the channel, we need to call an async close method.
    // Please see Drop impl.
    channel: Option<BoxedChannel>,
    ok_header_sent: bool,
}

impl Request {
    pub(crate) fn new(channel: BoxedChannel) -> Self {
        Self {
            channel: Some(channel),
            ok_header_sent: false,
        }
    }
}

impl RequestInterface for Request {
    fn reject(self, reason: RejectReason) {
        let mut us = self;
        let header: Vec<u8> = vec![Status::Failed(reason).into()];
        spawn!(
            async move {
                let channel = us.channel.as_mut().expect("Channel taken on drop");
                let _ = channel.send(header.into()).await;
            },
            "POOL: reject request"
        );
    }

    async fn send(&mut self, frame: Bytes) -> io::Result<()> {
        let channel = self.channel.as_mut().expect("Channel taken on drop");
        if !self.ok_header_sent {
            // We haven't sent the header.
            // Let's send it first and only once.
            let header: Vec<u8> = vec![Status::Ok.into()];
            channel.send(header.into()).await?;
            self.ok_header_sent = true;
        }
        channel.send(frame).await
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        // Close the channel gracefully.
        // This will close the underlying QUIC stream gracefully.
        let mut channel = self.channel.take().expect("Channel taken on drop");
        spawn!(
            async move {
                let _ = channel.close().await;
            },
            "POOL: drop request"
        );
    }
}

pub enum Status {
    Ok,
    Failed(RejectReason),
}

impl TryFrom<u8> for Status {
    type Error = std::io::Error;

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        let result = match value {
            0 => Self::Ok,
            1 => Self::Failed(RejectReason::TooManyRequests),
            2 => Self::Failed(RejectReason::ContentNotFound),
            3 => Self::Failed(RejectReason::Other),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid status value",
                ));
            },
        };

        Ok(result)
    }
}

impl From<Status> for u8 {
    fn from(value: Status) -> Self {
        match value {
            Status::Ok => 0,
            Status::Failed(RejectReason::TooManyRequests) => 1,
            Status::Failed(RejectReason::ContentNotFound) => 2,
            Status::Failed(RejectReason::Other) => 3,
            _ => unreachable!(),
        }
    }
}
