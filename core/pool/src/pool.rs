use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::Poll;

use anyhow::{Context, Result};
use async_trait::async_trait;
use bytes::Bytes;
use fleek_crypto::NodePublicKey;
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
use quinn::{RecvStream, SendStream};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use crate::builder::Builder;
use crate::config::Config;
use crate::endpoint::{self, Endpoint, Event, NodeAddress};

#[derive(Deserialize, Serialize)]
pub enum Status {
    Ok,
    Failed(RejectReason),
}

pub enum State {
    NotRunning { endpoint: Option<Endpoint> },
    Running { handle: JoinHandle<Endpoint> },
}

pub struct Pool {
    state: Mutex<Option<State>>,
}

impl Pool {
    fn new(endpoint: Endpoint) -> Result<Self> {
        Ok(Self {
            state: Mutex::new(Some(State::NotRunning {
                endpoint: Some(endpoint),
            })),
        })
    }

    fn register(&self, service: ServiceScope) -> (Sender<endpoint::Request>, Receiver<Event>) {
        let mut guard = self.state.blocking_lock();
        match guard.as_mut().expect("Pool to have a state") {
            State::Running { .. } => {
                panic!("failed to start: endpoint is already running");
            },
            State::NotRunning { endpoint } => {
                let endpoint = endpoint
                    .as_mut()
                    .expect("Endpoint to exist before registering");
                endpoint.register(service)
            },
        }
    }
}

#[async_trait]
impl WithStartAndShutdown for Pool {
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

impl ConfigConsumer for Pool {
    const KEY: &'static str = "pool";
    type Config = Config;
}

impl<C: Collection> PoolInterface<C> for Pool {
    type EventHandler = EventHandler;
    type Requester = Requester;
    type Responder = Responder;

    fn init(
        config: Self::Config,
        signer: &c!(C::SignerInterface),
        _: c!(C::ApplicationInterface::SyncExecutor),
        _: c!(C::NotifierInterface),
        _: c!(C::TopologyInterface),
    ) -> Result<Self> {
        let (_, sk) = signer.get_sk();
        let mut builder = Builder::new(sk);
        builder.keep_alive_interval(config.keep_alive_interval);
        builder.socket_address(config.address);
        let endpoint = builder.build()?;
        Pool::new(endpoint)
    }

    fn open_event(&self, service: ServiceScope) -> Self::EventHandler {
        let (tx, rx) = self.register(service);
        EventHandler {
            _event_rx: rx,
            _request_tx: tx,
            _service_scope: service,
        }
    }

    fn open_req_res(&self, service: ServiceScope) -> (Self::Requester, Self::Responder) {
        let (tx, rx) = self.register(service);
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
    _request_tx: Sender<endpoint::Request>,
    _event_rx: Receiver<Event>,
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

#[derive(Clone)]
pub struct Requester {
    request_tx: Sender<endpoint::Request>,
    service_scope: ServiceScope,
}

#[async_trait]
impl lightning_interfaces::pool::Requester for Requester {
    type Response = Response;

    async fn request(&self, _: NodeIndex, request: Bytes) -> Self::Response {
        let (tx, rx) = oneshot::channel();
        // Todo: Return a bad response if channel send fails.
        // Todo: Update service code.
        self.request_tx
            .send(endpoint::Request::NewStream {
                peer: NodeAddress {
                    pk: NodePublicKey([0; 32]),
                    socket_address: SocketAddr::from(([0, 0, 0, 0], 0)),
                },
                service: self.service_scope,
                respond: tx,
            })
            .await
            .unwrap();
        let (stream_tx, stream_rx) = rx.await.unwrap().unwrap();

        FramedWrite::new(stream_tx, LengthDelimitedCodec::new())
            .send(request)
            .await
            .unwrap();
        let mut response_rx = FramedRead::new(stream_rx, LengthDelimitedCodec::new());
        let header = response_rx.next().await.unwrap().unwrap();
        let status: Status = bincode::deserialize(header.as_ref()).unwrap();
        Response {
            status,
            rx: response_rx,
        }
    }
}

pub struct Response {
    status: Status,
    rx: FramedRead<RecvStream, LengthDelimitedCodec>,
}

#[async_trait]
impl lightning_interfaces::pool::Response for Response {
    type Body<S: Stream<Item = io::Result<Bytes>>> = Body;
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

pub struct Body {
    rx: FramedRead<RecvStream, LengthDelimitedCodec>,
}

impl Stream for Body {
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

pub struct Responder {
    inner: Receiver<Event>,
}

#[async_trait]
impl lightning_interfaces::pool::Responder for Responder {
    type Request = Request;

    async fn get_next_request(&mut self) -> io::Result<(Bytes, Self::Request)> {
        let request = self
            .inner
            .recv()
            .await
            .ok_or_else(|| io::Error::from(io::ErrorKind::BrokenPipe))?;
        let Event::NewStream { tx, rx, ..  } = request else {
            return Err(io::ErrorKind::Other.into());
        };
        let mut message_rx = FramedRead::new(rx, LengthDelimitedCodec::new());
        let bytes = message_rx
            .next()
            .await
            .ok_or(io::ErrorKind::BrokenPipe)?
            .map(Bytes::from)?;
        Ok((
            bytes,
            Request {
                ok_header_sent: false,
                stream_tx: FramedWrite::new(tx, LengthDelimitedCodec::new()),
            },
        ))
    }
}

pub struct Request {
    stream_tx: FramedWrite<SendStream, LengthDelimitedCodec>,
    ok_header_sent: bool,
}

#[async_trait]
impl lightning_interfaces::pool::Request for Request {
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
