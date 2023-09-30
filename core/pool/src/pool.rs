use std::io;
use std::pin::Pin;
use std::task::Poll;

use anyhow::{Context, Result};
use async_trait::async_trait;
use bytes::Bytes;
use futures::{SinkExt, Stream, StreamExt};
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::pool::{PoolInterface, RejectReason, ServiceScope};
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{
    ApplicationInterface,
    ConfigConsumer,
    SignerInterface,
    WithStartAndShutdown,
};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use crate::builder::Builder;
use crate::config::Config;
use crate::connection::ConnectionPool;
use crate::service::broadcast::BroadcastRequest;
use crate::service::stream::StreamRequest;

#[derive(Deserialize, Serialize)]
pub enum Status {
    Ok,
    Failed(RejectReason),
}

pub enum State<C: Collection, W, R> {
    NotRunning {
        endpoint: Option<ConnectionPool<C, W, R>>,
    },
    Running {
        handle: JoinHandle<ConnectionPool<C, W, R>>,
    },
}

pub struct Pool<C: Collection, W, R> {
    state: Mutex<Option<State<C, W, R>>>,
}

impl<C, W, R> Pool<C, W, R>
where
    C: Collection,
    W: AsyncWrite + Send + Sync + Unpin + 'static,
    R: AsyncRead + Send + Sync + Unpin + 'static,
{
    fn new(endpoint: ConnectionPool<C, W, R>) -> Result<Self> {
        Ok(Self {
            state: Mutex::new(Some(State::NotRunning {
                endpoint: Some(endpoint),
            })),
        })
    }

    fn register_broadcast_service(
        &self,
        service: ServiceScope,
    ) -> (
        Sender<BroadcastRequest<Box<dyn Fn(NodeIndex) -> bool + Send + Sync + 'static>>>,
        Receiver<Bytes>,
    ) {
        let mut guard = self.state.blocking_lock();
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
    ) -> (Sender<StreamRequest<W, R>>, Receiver<(W, R)>) {
        let mut guard = self.state.blocking_lock();
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
impl<C, W, R> WithStartAndShutdown for Pool<C, W, R>
where
    C: Collection,
    W: AsyncWrite + Send + Sync + Unpin + 'static,
    R: AsyncRead + Send + Sync + Unpin + 'static,
{
    fn is_running(&self) -> bool {
        matches!(
            self.state.blocking_lock().as_ref(),
            Some(&State::Running { .. })
        )
    }

    async fn start(&self) {
        let mut guard = self.state.lock().await;
        let state = guard.take().expect("There to be a state");
        let handle = match state {
            State::Running { .. } => {
                panic!("failed to start: endpoint is already running");
            },
            State::NotRunning { mut endpoint } => {
                let mut endpoint = endpoint.take().expect("There to be an Endpoint");
                tokio::spawn(async move {
                    if let Err(e) = endpoint.start().await {
                        log::error!("unexpected endpoint failure: {e:?}");
                    }
                    endpoint
                })
            },
        };
        *guard = Some(State::Running { handle });
    }

    async fn shutdown(&self) {
        let mut guard = self.state.lock().await;
        let state = guard.take().expect("There to be a state");
        let endpoint = match state {
            State::Running { handle } => handle.await.context("endpoint tasked failed").unwrap(),
            State::NotRunning { .. } => {
                panic!("failed to shutdown: endpoint is not running");
            },
        };
        *guard = Some(State::NotRunning {
            endpoint: Some(endpoint),
        });
    }
}

impl<C, W, R> ConfigConsumer for Pool<C, W, R>
where
    C: Collection,
    W: AsyncWrite + Send + Sync + Unpin + 'static,
    R: AsyncRead + Send + Sync + Unpin + 'static,
{
    const KEY: &'static str = "pool";
    type Config = Config;
}

impl<C, W, R> PoolInterface<C> for Pool<C, W, R>
where
    C: Collection,
    W: AsyncWrite + Send + Sync + Unpin + 'static,
    R: AsyncRead + Send + Sync + Unpin + 'static,
{
    type EventHandler = EventHandler;
    type Requester = Requester<W, R>;
    type Responder = Responder<W, R>;

    fn init(
        config: Self::Config,
        signer: &c!(C::SignerInterface),
        sync_query: c!(C::ApplicationInterface::SyncExecutor),
        _: c!(C::NotifierInterface),
        topology: c!(C::TopologyInterface),
    ) -> Result<Self> {
        let (_, sk) = signer.get_sk();
        let mut builder = Builder::new(sk, topology, sync_query);
        builder.keep_alive_interval(config.keep_alive_interval);
        builder.socket_address(config.address);
        let endpoint = builder.build()?;
        Pool::new(endpoint)
    }

    fn open_event(&self, service: ServiceScope) -> Self::EventHandler {
        let (tx, rx) = self.register_broadcast_service(service);
        EventHandler {
            _event_rx: rx,
            _request_tx: tx,
            _service_scope: service,
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
    _request_tx: Sender<BroadcastRequest<Box<dyn Fn(NodeIndex) -> bool + Send + Sync + 'static>>>,
    _event_rx: Receiver<Bytes>,
    _service_scope: ServiceScope,
}

#[async_trait]
impl lightning_interfaces::pool::EventHandler for EventHandler {
    fn send_to_all<F: Fn(NodeIndex) -> bool>(&self, _: Bytes, _: F) {
        todo!()
    }

    fn send_to_one(&self, _: NodeIndex, _: Bytes) {
        todo!()
    }

    async fn receive(&mut self) -> Option<(NodeIndex, Bytes)> {
        todo!()
    }
}

pub struct Requester<W, R> {
    request_tx: Sender<StreamRequest<W, R>>,
    service_scope: ServiceScope,
}

impl<W, R> Clone for Requester<W, R> {
    fn clone(&self) -> Self {
        Self {
            request_tx: self.request_tx.clone(),
            service_scope: self.service_scope,
        }
    }
}

#[async_trait]
impl<W, R> lightning_interfaces::pool::Requester for Requester<W, R>
where
    W: AsyncWrite + Send + Sync + Unpin + 'static,
    R: AsyncRead + Send + Sync + Unpin + 'static,
{
    type Response = Response<R>;

    async fn request(&self, _: NodeIndex, request: Bytes) -> io::Result<Self::Response> {
        let (tx, rx) = oneshot::channel();
        // Todo: Return a bad response if channel send fails.
        // Todo: Update service code.
        self.request_tx
            .send(StreamRequest { respond: tx })
            .await
            .unwrap();
        let (stream_tx, stream_rx) = rx.await.map_err(|_| io::ErrorKind::BrokenPipe)?;

        FramedWrite::new(stream_tx, LengthDelimitedCodec::new())
            .send(request)
            .await
            .unwrap();
        let mut response_rx = FramedRead::new(stream_rx, LengthDelimitedCodec::new());
        let header = response_rx.next().await.unwrap().unwrap();
        let status: Status = bincode::deserialize(header.as_ref()).unwrap();
        Ok(Response {
            status,
            rx: response_rx,
        })
    }
}

pub struct Response<R>
where
    R: AsyncRead + Send + Sync + Unpin + 'static,
{
    status: Status,
    rx: FramedRead<R, LengthDelimitedCodec>,
}

#[async_trait]
impl<R> lightning_interfaces::pool::Response for Response<R>
where
    R: AsyncRead + Send + Sync + Unpin + 'static,
{
    type Body<S: Stream<Item = io::Result<Bytes>>> = Body<R>;
    fn status_code(&self) -> Result<(), RejectReason> {
        match &self.status {
            Status::Ok => Ok(()),
            Status::Failed(reason) => Err(*reason),
        }
    }

    fn body<S: Stream<Item = io::Result<Bytes>>>(self) -> Self::Body<S> {
        Body { rx: self.rx }
    }
}

pub struct Body<R> {
    rx: FramedRead<R, LengthDelimitedCodec>,
}

impl<R> Stream for Body<R>
where
    R: AsyncRead + Send + Sync + Unpin + 'static,
{
    type Item = io::Result<Bytes>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.rx)
            .poll_next(cx)
            .map(|item| item.map(|result| result.map(|bytes| Bytes::from(bytes.to_vec()))))
            .map_err(Into::into)
    }
}

pub struct Responder<W, R> {
    inner: Receiver<(W, R)>,
}

#[async_trait]
impl<W, R> lightning_interfaces::pool::Responder for Responder<W, R>
where
    W: AsyncWrite + Send + Sync + Unpin + 'static,
    R: AsyncRead + Send + Sync + Unpin + 'static,
{
    type Request = Request<W>;

    async fn get_next_request(&mut self) -> io::Result<(Bytes, Self::Request)> {
        let (stream_tx, stream_rx) = self
            .inner
            .recv()
            .await
            .ok_or_else(|| io::Error::from(io::ErrorKind::BrokenPipe))?;
        let mut message_rx = FramedRead::new(stream_rx, LengthDelimitedCodec::new());
        let bytes = message_rx
            .next()
            .await
            .ok_or(io::ErrorKind::BrokenPipe)?
            .map(Bytes::from)?;
        Ok((
            bytes,
            Request {
                ok_header_sent: false,
                stream_tx: FramedWrite::new(stream_tx, LengthDelimitedCodec::new()),
            },
        ))
    }
}

pub struct Request<W> {
    stream_tx: FramedWrite<W, LengthDelimitedCodec>,
    ok_header_sent: bool,
}

#[async_trait]
impl<W> lightning_interfaces::pool::Request for Request<W>
where
    W: AsyncWrite + Send + Sync + Unpin + 'static,
{
    fn reject(self, reason: RejectReason) {
        let mut us = self;
        let header = bincode::serialize(&Status::Failed(reason)).expect("Defined object");
        tokio::spawn(async move { us.stream_tx.send(Bytes::from(header)).await });
    }

    // Todo: Maybe this doesnt have to be mut. Let's think about it more.
    async fn send(&mut self, frame: Bytes) -> io::Result<()> {
        if !self.ok_header_sent {
            let header = bincode::serialize(&Status::Ok)
                .map(Bytes::from)
                .expect("Defined object");
            self.stream_tx.send(header).await?;
            self.ok_header_sent = true;
        }
        self.stream_tx.send(frame).await
    }
}
