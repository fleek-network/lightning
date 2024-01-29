mod config;
mod handler;

use async_channel::{Receiver, Sender};
use async_trait::async_trait;
use axum::routing::get;
use axum::Router;
use bytes::Bytes;
pub use config::Config;
use lightning_interfaces::ExecutorProviderInterface;
use lightning_schema::handshake::{
    HandshakeRequestFrame,
    HandshakeResponse,
    RequestFrame,
    ResponseFrame,
    TerminationReason,
};
use tokio::sync::oneshot;
use tracing::warn;

use crate::shutdown::ShutdownWaiter;
use crate::transports::{Transport, TransportReceiver, TransportSender};

pub struct HttpTransport {}

#[async_trait]
impl Transport for HttpTransport {
    type Config = Config;
    type Sender = HttpSender;
    type Receiver = HttpReceiver;

    async fn bind<P: ExecutorProviderInterface>(
        _: ShutdownWaiter,
        _: Self::Config,
    ) -> anyhow::Result<(Self, Option<Router>)> {
        let router = Router::new()
            .route(
                "/services/0/:origin/:uri",
                get(handler::fetcher_service_handler::<P>),
            )
            .route(
                "/services/1/:origin/:uri",
                get(handler::js_service_handler::<P>),
            );
        Ok((Self {}, Some(router)))
    }

    async fn accept(&mut self) -> Option<(HandshakeRequestFrame, Self::Sender, Self::Receiver)> {
        unreachable!()
    }
}

pub enum Service {
    Fetcher,
    Js,
}

pub struct HttpSender {
    service: Service,
    frame_tx: Sender<Option<RequestFrame>>,
    body_tx: Sender<anyhow::Result<Bytes>>,
    expected_block_count: Option<usize>,
    current_block_len: usize,
    termination_tx: Option<oneshot::Sender<TerminationReason>>,
}

impl HttpSender {
    pub fn new(
        service: Service,
        frame_tx: Sender<Option<RequestFrame>>,
        body_tx: Sender<anyhow::Result<Bytes>>,
        termination_tx: oneshot::Sender<TerminationReason>,
    ) -> Self {
        Self {
            service,
            frame_tx,
            body_tx,
            expected_block_count: None,
            current_block_len: 0,
            termination_tx: Some(termination_tx),
        }
    }

    #[inline(always)]
    fn inner_send(&mut self, bytes: Bytes) {
        if let Err(e) = self.body_tx.try_send(Ok(bytes)) {
            warn!("payload dropped, failed to send to write loop: {e}");
        }
    }

    #[inline(always)]
    fn close(&mut self) {
        if self.frame_tx.try_send(None).is_err() {
            warn!("failed to send close signal to receiver");
        }
    }

    fn fetcher_service_write(&mut self, buf: Bytes) -> anyhow::Result<usize> {
        let len = buf.len();
        self.current_block_len -= len;

        if self.expected_block_count.is_none() && len == 4 {
            let expected_block_count = u32::from_be_bytes(buf.as_ref().try_into()?);
            assert!(
                self.expected_block_count
                    .replace(expected_block_count as usize)
                    .is_none()
            );
            return Ok(4);
        }

        self.inner_send(buf);

        let expected_block_count = self
            .expected_block_count
            .as_ref()
            .expect("To be initialized");
        if *expected_block_count == 0 && self.current_block_len == 0 {
            self.close();
        }

        Ok(len)
    }

    fn js_service_write(&mut self, buf: Bytes) -> anyhow::Result<usize> {
        let len = buf.len();
        self.inner_send(buf);
        self.current_block_len -= len;

        if self.current_block_len == 0 {
            self.close();
        }

        Ok(len)
    }
}

impl TransportSender for HttpSender {
    fn send_handshake_response(&mut self, _: HandshakeResponse) {
        unimplemented!()
    }

    fn send(&mut self, _: ResponseFrame) {
        unimplemented!()
    }

    fn terminate(mut self, reason: TerminationReason) {
        if let Some(reason_sender) = self.termination_tx.take() {
            let _ = reason_sender.send(reason);
        }
    }

    fn start_write(&mut self, len: usize) {
        self.termination_tx.take();

        debug_assert!(
            self.current_block_len == 0,
            "data should be written completely before calling start_write again"
        );

        self.current_block_len = len;

        if let Service::Fetcher = self.service {
            if let Some(expected_block_count) = self.expected_block_count.as_mut() {
                *expected_block_count -= 1;
            }
        }
    }

    fn write(&mut self, buf: Bytes) -> anyhow::Result<usize> {
        debug_assert!(self.current_block_len != 0);
        debug_assert!(self.current_block_len >= buf.len());

        match self.service {
            Service::Fetcher => self.fetcher_service_write(buf),
            Service::Js => self.js_service_write(buf),
        }
    }
}

pub struct HttpReceiver {
    inner: Receiver<Option<RequestFrame>>,
}

impl HttpReceiver {
    pub fn new(inner: Receiver<Option<RequestFrame>>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl TransportReceiver for HttpReceiver {
    async fn recv(&mut self) -> Option<RequestFrame> {
        self.inner.recv().await.ok().flatten()
    }
}
