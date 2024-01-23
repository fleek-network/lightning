use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::Poll;

use anyhow::{Context, Result};
use bytes::Bytes;
use fleek_crypto::SecretKey;
use futures::{SinkExt, Stream};
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::pool::{PoolInterface, RejectReason, ServiceScope};
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{
    ApplicationInterface,
    ConfigConsumer,
    EventHandlerInterface,
    ReputationAggregatorInterface,
    RequestHeader,
    RequestInterface,
    RequesterInterface,
    ResponderInterface,
    ResponseInterface,
    SignerInterface,
    WithStartAndShutdown,
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::endpoint::{Endpoint, EndpointTask};
use crate::event::{Event, EventReceiver, Param};
use crate::muxer::quinn::QuinnMuxer;
use crate::muxer::{BoxedChannel, MuxerInterface};
use crate::{http, muxer, tls};

pub struct PoolProvider<C, M = QuinnMuxer>
where
    C: Collection,
    M: MuxerInterface,
{
    state: Mutex<Option<State<C, M>>>,
    event_queue: Sender<Event>,
    endpoint_task_queue: Sender<EndpointTask>,
    config: Config,
    shutdown: CancellationToken,
}

impl<C, M> PoolProvider<C, M>
where
    C: Collection,
    M: MuxerInterface,
{
    fn new(
        endpoint: Endpoint<C, M>,
        receiver: EventReceiver<C>,
        event_queue: Sender<Event>,
        endpoint_task_queue: Sender<EndpointTask>,
        config: Config,
        shutdown: CancellationToken,
    ) -> Result<Self> {
        Ok(Self {
            state: Mutex::new(Some(State::NotRunning { endpoint, receiver })),
            event_queue,
            endpoint_task_queue,
            config,
            shutdown,
        })
    }

    fn register_broadcast_service(
        &self,
        service: ServiceScope,
    ) -> (Sender<Event>, Receiver<(NodeIndex, Bytes)>) {
        let mut guard = self.state.lock().unwrap();
        match guard.as_mut().expect("Pool to have a state") {
            State::Running { .. } => {
                panic!("failed to start: endpoint is already running");
            },
            State::NotRunning { receiver, .. } => (
                self.event_queue.clone(),
                receiver.register_broadcast_service(service),
            ),
        }
    }

    fn register_requester_service(
        &self,
        service: ServiceScope,
    ) -> (Sender<Event>, Receiver<(RequestHeader, Request)>) {
        let mut guard = self.state.lock().unwrap();
        match guard.as_mut().expect("Pool to have a state") {
            State::Running { .. } => {
                panic!("failed to start: endpoint is already running");
            },
            State::NotRunning { receiver, .. } => (
                self.event_queue.clone(),
                receiver.register_requester_service(service),
            ),
        }
    }
}

impl<C> WithStartAndShutdown for PoolProvider<C, QuinnMuxer>
where
    C: Collection,
{
    fn is_running(&self) -> bool {
        matches!(
            self.state.lock().unwrap().as_ref(),
            Some(&State::Running { .. })
        )
    }

    async fn start(&self) {
        let state = {
            let mut guard = self.state.lock().unwrap();
            guard.take().expect("There to be a state")
        };

        let next_state = if let State::NotRunning { endpoint, receiver } = state {
            if let Some(http_server_address) = self.config.http {
                let shutdown_http_server = self.shutdown.clone();
                let event_tx = self.event_queue.clone();
                let endpoint_task_tx = self.endpoint_task_queue.clone();

                tokio::spawn(async move {
                    http::spawn_http_server(
                        http_server_address,
                        shutdown_http_server,
                        event_tx,
                        endpoint_task_tx,
                    )
                    .await
                });
            }

            State::Running {
                receiver_handle: receiver.spawn(),
                endpoint_handle: endpoint.spawn(),
            }
        } else {
            state
        };

        let mut guard = self.state.lock().unwrap();
        *guard = Some(next_state);
    }

    async fn shutdown(&self) {
        let state = {
            let mut guard = self.state.lock().unwrap();
            guard.take().expect("There to be a state")
        };

        let next_state = if let State::Running {
            endpoint_handle,
            receiver_handle,
        } = state
        {
            self.shutdown.cancel();
            // We shutdown the endpoint first because it will close
            // the ongoing connection loops that receive input from
            // the network which in turn may enqueue events in the receiver
            // queue, preventing it from shutting down.
            let mut endpoint = endpoint_handle
                .await
                .context("endpoint tasked failed")
                .unwrap();
            endpoint.shutdown().await;
            let mut receiver = receiver_handle
                .await
                .context("event receiver tasked failed")
                .unwrap();
            receiver.shutdown().await;

            State::NotRunning { endpoint, receiver }
        } else {
            state
        };

        let mut guard = self.state.lock().unwrap();
        *guard = Some(next_state);
    }
}

impl<C> ConfigConsumer for PoolProvider<C, QuinnMuxer>
where
    C: Collection,
{
    const KEY: &'static str = "pool";
    type Config = Config;
}

// Todo: An improvement would be to pass a `Muxer` in `init`.
// See comments in `MuxerInterface`.
impl<C: Collection> PoolInterface<C> for PoolProvider<C, QuinnMuxer> {
    type EventHandler = EventHandler;
    type Requester = Requester;
    type Responder = Responder;

    fn init(
        config: Self::Config,
        signer: &c!(C::SignerInterface),
        sync_query: c!(C::ApplicationInterface::SyncExecutor),
        notifier: c!(C::NotifierInterface),
        topology: c!(C::TopologyInterface),
        _: c!(C::ReputationAggregatorInterface::ReputationReporter),
    ) -> Result<PoolProvider<C, QuinnMuxer>> {
        let (_, sk) = signer.get_sk();
        let public_key = sk.to_pk();

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

        let shutdown = CancellationToken::new();
        let (endpoint_task_tx, endpoint_task_rx) = mpsc::channel(1024);
        let (event_tx, event_rx) = mpsc::channel(1024);
        let receiver = EventReceiver::<C>::new(
            sync_query.clone(),
            topology,
            notifier,
            event_rx,
            endpoint_task_tx.clone(),
            public_key,
            shutdown.clone(),
        );
        let endpoint = Endpoint::<C, QuinnMuxer>::new(
            sync_query,
            endpoint_task_rx,
            event_tx.clone(),
            muxer_config,
            shutdown.clone(),
        );

        PoolProvider::new(
            endpoint,
            receiver,
            event_tx,
            endpoint_task_tx,
            config,
            shutdown,
        )
    }

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
        tokio::spawn(async move {
            if request_tx
                .send(Event::Broadcast {
                    service_scope,
                    message,
                    param: Param::Filter(Box::new(filter)),
                })
                .await
                .is_err()
            {
                tracing::error!("failed to send to broadcast service");
            }
        });
    }

    fn send_to_one(&self, index: NodeIndex, message: Bytes) {
        let request_tx = self.request_tx.clone();
        let service_scope = self.service_scope;
        tokio::spawn(async move {
            if request_tx
                .send(Event::Broadcast {
                    service_scope,
                    message,
                    param: Param::Index(index),
                })
                .await
                .is_err()
            {
                tracing::error!("failed to send to broadcast service");
            }
        });
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
        let header = bincode::serialize(&Status::Failed(reason)).expect("Typed object");
        tokio::spawn(async move {
            let channel = us.channel.as_mut().expect("Channel taken on drop");
            let _ = channel.send(Bytes::from(header)).await;
        });
    }

    async fn send(&mut self, frame: Bytes) -> io::Result<()> {
        let channel = self.channel.as_mut().expect("Channel taken on drop");
        if !self.ok_header_sent {
            // We haven't sent the header.
            // Let's send it first and only once.
            let header = bincode::serialize(&Status::Ok)
                .map(Bytes::from)
                .expect("Defined object");
            channel.send(header).await?;
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
        tokio::spawn(async move {
            let _ = channel.close().await;
        });
    }
}

#[derive(Deserialize, Serialize)]
pub enum Status {
    Ok,
    Failed(RejectReason),
}

pub enum State<C, M>
where
    C: Collection,
    M: MuxerInterface,
{
    NotRunning {
        receiver: EventReceiver<C>,
        endpoint: Endpoint<C, M>,
    },
    Running {
        receiver_handle: JoinHandle<EventReceiver<C>>,
        endpoint_handle: JoinHandle<Endpoint<C, M>>,
    },
}
