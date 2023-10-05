use std::cell::OnceCell;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use bytes::Bytes;
use fleek_crypto::SecretKey;
use futures::{SinkExt, Stream, StreamExt};
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::pool::{PoolInterface, RejectReason, ServiceScope};
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{
    ApplicationInterface,
    ConfigConsumer,
    NotifierInterface,
    SignerInterface,
    WithStartAndShutdown,
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot, Notify};
use tokio::task::JoinHandle;

use crate::config::Config;
use crate::endpoint::Endpoint;
use crate::muxer::quinn::QuinnMuxer;
use crate::muxer::{Channel, MuxerInterface};
use crate::service::broadcast::{BroadcastRequest, Param};
use crate::service::stream::StreamRequest;
use crate::{muxer, tls};

pub struct Pool<C, M = QuinnMuxer>
where
    C: Collection,
    M: MuxerInterface,
{
    state: Mutex<Option<State<C, M>>>,
    shutdown_notify: Arc<Notify>,
}

impl<C, M> Pool<C, M>
where
    C: Collection,
    M: MuxerInterface,
{
    fn new(endpoint: Endpoint<C, M>) -> Result<Self> {
        Ok(Self {
            state: Mutex::new(Some(State::NotRunning {
                endpoint: Some(endpoint),
            })),
            shutdown_notify: Arc::new(Notify::new()),
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

    fn register_stream_service(
        &self,
        service: ServiceScope,
    ) -> (Sender<StreamRequest>, Receiver<Channel>) {
        let mut guard = self.state.lock().unwrap();
        match guard.as_mut().expect("Pool to have a state") {
            State::Running { .. } => {
                panic!("failed to start: endpoint is already running");
            },
            State::NotRunning { endpoint } => {
                let endpoint = endpoint
                    .as_mut()
                    .expect("Endpoint to exist before registering");
                endpoint.register_stream_service(service)
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
        let shutdown = self.shutdown_notify.clone();
        let mut guard = self.state.lock().unwrap();
        let state = guard.take().expect("There to be a state");
        let handle = match state {
            State::Running { .. } => {
                panic!("failed to start: endpoint is already running");
            },
            State::NotRunning { mut endpoint } => {
                let mut endpoint = endpoint.take().expect("There to be an Endpoint");
                tokio::spawn(async move {
                    if let Err(e) = endpoint.start(shutdown).await {
                        log::error!("unexpected endpoint failure: {e:?}");
                    }
                    endpoint
                })
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
                self.shutdown_notify.notify_one();
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
    ) -> Result<Pool<C, QuinnMuxer>> {
        let (_, sk) = signer.get_sk();
        let public_key = sk.to_pk();

        let mut transport_config = quinn::TransportConfig::default();
        transport_config.max_idle_timeout(Some(
            Duration::from_secs(config.max_idle_timeout).try_into()?,
        ));
        let tls_config = tls::make_server_config(&sk).expect("Secret key to be valid");
        let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(tls_config));
        server_config.transport_config(Arc::new(transport_config));

        let muxer_config = muxer::quinn::Config {
            server_config,
            address: config.address,
            sk,
        };

        let (notifier_tx, notifier_rx) = mpsc::channel(32);
        notifier.notify_on_new_epoch(notifier_tx);

        Pool::new(Endpoint::<C, QuinnMuxer>::new(
            topology,
            sync_query,
            notifier_rx,
            muxer_config,
            public_key,
            OnceCell::new(),
        ))
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
        let (tx, rx) = self.register_stream_service(service);
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
impl lightning_interfaces::pool::EventHandler for EventHandler {
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

    async fn receive(&mut self) -> Option<(NodeIndex, Bytes)> {
        self.event_rx.recv().await
    }
}

pub struct Requester {
    request_tx: Sender<StreamRequest>,
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
impl lightning_interfaces::pool::Requester for Requester {
    type Response = Response;

    async fn request(&self, peer: NodeIndex, request: Bytes) -> io::Result<Self::Response> {
        let (respond_tx, respond_rx) = oneshot::channel();
        // Request a stream.
        self.request_tx
            .send(StreamRequest {
                peer,
                service_scope: self.service_scope,
                respond: respond_tx,
            })
            .await
            .map_err(|_| io::ErrorKind::BrokenPipe)?;

        let mut channel = respond_rx.await.map_err(|_| io::ErrorKind::BrokenPipe)??;

        // Send our request.
        channel.send(request).await?;

        // Read the response header.
        let header = channel.next().await.ok_or(io::ErrorKind::BrokenPipe)??;

        // Todo: Use something better than bincode.
        let status: Status =
            bincode::deserialize(header.as_ref()).map_err(|_| io::ErrorKind::Other)?;

        Ok(Response { status, channel })
    }
}

pub struct Response {
    status: Status,
    channel: Channel,
}

#[async_trait]
impl lightning_interfaces::pool::Response for Response {
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
    channel: Channel,
}

impl Stream for Body {
    type Item = io::Result<Bytes>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.channel)
            .poll_next(cx)
            .map(|item| item.map(|result| result.map(|bytes| Bytes::from(bytes.to_vec()))))
            .map_err(Into::into)
    }
}

pub struct Responder {
    inner: Receiver<Channel>,
}

#[async_trait]
impl lightning_interfaces::pool::Responder for Responder {
    type Request = Request;

    async fn get_next_request(&mut self) -> io::Result<(Bytes, Self::Request)> {
        // Received a new stream so a request is incoming.
        let mut channel = self
            .inner
            .recv()
            .await
            .ok_or_else(|| io::Error::from(io::ErrorKind::BrokenPipe))?;

        // Get the header.
        let bytes = channel
            .next()
            .await
            .ok_or(io::ErrorKind::BrokenPipe)?
            .map(Bytes::from)?;

        Ok((
            bytes,
            Request {
                ok_header_sent: false,
                channel,
            },
        ))
    }
}

pub struct Request {
    channel: Channel,
    ok_header_sent: bool,
}

#[async_trait]
impl lightning_interfaces::pool::Request for Request {
    fn reject(self, reason: RejectReason) {
        let mut us = self;
        let header = bincode::serialize(&Status::Failed(reason)).expect("Typed object");
        tokio::spawn(async move { us.channel.send(Bytes::from(header)).await });
    }

    async fn send(&mut self, frame: Bytes) -> io::Result<()> {
        if !self.ok_header_sent {
            // We haven't sent the header.
            // Let's send it first and only once.
            let header = bincode::serialize(&Status::Ok)
                .map(Bytes::from)
                .expect("Defined object");
            self.channel.send(header).await?;
            self.ok_header_sent = true;
        }
        self.channel.send(frame).await
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
