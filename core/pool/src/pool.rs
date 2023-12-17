use std::cell::OnceCell;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::Poll;

use anyhow::{Context, Result};
use async_trait::async_trait;
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
    NotifierInterface,
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
use crate::endpoint::Endpoint;
use crate::muxer::quinn::QuinnMuxer;
use crate::muxer::{BoxedChannel, MuxerInterface};
use crate::overlay::{BroadcastRequest, Param, SendRequest};
use crate::state::QuerySender;
use crate::{http, muxer, tls};

pub struct Pool<C, M = QuinnMuxer>
where
    C: Collection,
    M: MuxerInterface,
{
    state: Mutex<Option<State<C, M>>>,
    config: Config,
    state_request: QuerySender,
    shutdown: CancellationToken,
}

impl<C, M> Pool<C, M>
where
    C: Collection,
    M: MuxerInterface,
{
    fn new(endpoint: Endpoint<C, M>, config: Config, state_request: QuerySender) -> Result<Self> {
        Ok(Self {
            state: Mutex::new(Some(State::NotRunning {
                endpoint: Some(endpoint),
            })),
            config,
            state_request,
            shutdown: CancellationToken::new(),
        })
    }

    fn register_broadcast_service(
        &self,
        service: ServiceScope,
    ) -> (Sender<BroadcastRequest>, Receiver<(NodeIndex, Bytes)>) {
        let mut guard = self.state.lock().unwrap();
        match guard.as_mut().expect("Pool to have a state") {
            State::Running { .. } => {
                panic!("failed to start: endpoint is already running");
            },
            State::NotRunning { endpoint } => {
                let endpoint = endpoint
                    .as_mut()
                    .expect("Endpoint to exist before registering");
                endpoint.register_broadcast_service(service)
            },
        }
    }

    fn register_requester_service(
        &self,
        service: ServiceScope,
    ) -> (Sender<SendRequest>, Receiver<(RequestHeader, Request)>) {
        let mut guard = self.state.lock().unwrap();
        match guard.as_mut().expect("Pool to have a state") {
            State::Running { .. } => {
                panic!("failed to start: endpoint is already running");
            },
            State::NotRunning { endpoint } => {
                let endpoint = endpoint
                    .as_mut()
                    .expect("Endpoint to exist before registering");
                endpoint.register_requester_service(service)
            },
        }
    }
}

#[async_trait]
impl<C> WithStartAndShutdown for Pool<C, QuinnMuxer>
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
        let mut guard = self.state.lock().unwrap();
        let state = guard.take().expect("There to be a state");
        let handle = match state {
            State::Running { .. } => {
                panic!("failed to start: endpoint is already running");
            },
            State::NotRunning { mut endpoint } => {
                let endpoint = endpoint.take().expect("There to be an Endpoint");
                let shutdown = self.shutdown.clone();

                if let Some(http_server_address) = self.config.http {
                    let shutdown_http_server = shutdown.clone();
                    let state_request = self.state_request.clone();
                    tokio::spawn(async move {
                        http::spawn_http_server(
                            http_server_address,
                            shutdown_http_server,
                            state_request,
                        )
                        .await
                    });
                }

                endpoint.spawn(shutdown)
            },
        };
        *guard = Some(State::Running { handle });
    }

    async fn shutdown(&self) {
        let state = {
            let mut guard = self.state.lock().unwrap();
            guard.take().expect("There to be a state")
        };

        let mut endpoint = match state {
            State::Running { handle } => {
                self.shutdown.cancel();
                handle.await.context("endpoint tasked failed").unwrap()
            },
            State::NotRunning { .. } => {
                panic!("failed to shutdown: endpoint is not running");
            },
        };

        endpoint.shutdown().await;

        let mut guard = self.state.lock().unwrap();
        *guard = Some(State::NotRunning {
            endpoint: Some(endpoint),
        });
    }
}

impl<C> ConfigConsumer for Pool<C, QuinnMuxer>
where
    C: Collection,
{
    const KEY: &'static str = "pool";
    type Config = Config;
}

// Todo: An improvement would be to pass a `Muxer` in `init`.
// See comments in `MuxerInterface`.
impl<C: Collection> PoolInterface<C> for Pool<C, QuinnMuxer> {
    type EventHandler = EventHandler;
    type Requester = Requester;
    type Responder = Responder;

    fn init(
        config: Self::Config,
        signer: &c!(C::SignerInterface),
        sync_query: c!(C::ApplicationInterface::SyncExecutor),
        notifier: c!(C::NotifierInterface),
        topology: c!(C::TopologyInterface),
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
    ) -> Result<Pool<C, QuinnMuxer>> {
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

        let (notifier_tx, notifier_rx) = mpsc::channel(32);
        notifier.notify_on_new_epoch(notifier_tx);

        let (state_request_tx, state_request_rx) = mpsc::channel(2);

        Pool::new(
            Endpoint::<C, QuinnMuxer>::new(
                topology,
                sync_query,
                notifier_rx,
                rep_reporter,
                muxer_config,
                public_key,
                OnceCell::new(),
                state_request_rx,
            ),
            config,
            state_request_tx,
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
    request_tx: Sender<BroadcastRequest>,
    event_rx: Receiver<(NodeIndex, Bytes)>,
    service_scope: ServiceScope,
}

#[async_trait]
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
                .send(BroadcastRequest {
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
                .send(BroadcastRequest {
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
    request_tx: Sender<SendRequest>,
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

#[async_trait]
impl RequesterInterface for Requester {
    type Response = Response;

    async fn request(&self, peer: NodeIndex, request: Bytes) -> io::Result<Self::Response> {
        let (respond_tx, respond_rx) = oneshot::channel();
        // Request a stream.
        self.request_tx
            .send(SendRequest {
                peer,
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

#[async_trait]
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

#[async_trait]
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

#[async_trait]
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
    NotRunning { endpoint: Option<Endpoint<C, M>> },
    Running { handle: JoinHandle<Endpoint<C, M>> },
}
