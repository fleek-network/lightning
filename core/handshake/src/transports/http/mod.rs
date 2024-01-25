mod config;

use std::collections::HashMap;
use std::fmt::Display;
use std::str::FromStr;

use async_trait::async_trait;
use axum::body::StreamBody;
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Extension, Router};
use base64::Engine;
use bytes::Bytes;
pub use config::Config;
use fleek_crypto::{ClientPublicKey, ClientSignature};
use lightning_interfaces::ExecutorProviderInterface;
use lightning_schema::handshake::{
    HandshakeRequestFrame,
    HandshakeResponse,
    RequestFrame,
    ResponseFrame,
    TerminationReason,
};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;
use tracing::warn;

use crate::handshake::Context;
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
        let router = Router::new().route("/services/:id", get(handler::<P>));
        Ok((Self {}, Some(router)))
    }

    async fn accept(&mut self) -> Option<(HandshakeRequestFrame, Self::Sender, Self::Receiver)> {
        unreachable!()
    }
}

async fn handler<P: ExecutorProviderInterface>(
    Path(id): Path<u32>,
    Query(params): Query<HashMap<String, String>>,
    Extension(provider): Extension<Context<P>>,
) -> Result<StreamBody<ReceiverStream<anyhow::Result<Bytes>>>, (StatusCode, String)> {
    tracing::trace!("received HTTP request for service {id:?}");

    let pk = match params.get("pk") {
        Some(pk) => {
            let bytes: [u8; 96] = base64::prelude::BASE64_STANDARD
                .decode(pk)
                .map_err(|_| bad_request("invalid pk value"))?
                .try_into()
                .map_err(|_| bad_request("invalid pk value"))?;
            ClientPublicKey(bytes)
        },
        None => return Err(bad_request("missing pk value")),
    };
    let pop = match params.get("pop") {
        Some(pop) => {
            let bytes: [u8; 48] = base64::prelude::BASE64_STANDARD
                .decode(pop)
                .map_err(|_| bad_request("invalid pop value"))?
                .try_into()
                .map_err(|_| bad_request("invalid pop value"))?;
            ClientSignature(bytes)
        },
        None => return Err(bad_request("missing pop value")),
    };
    let payload = match params.get("payload") {
        Some(payload) => base64::prelude::BASE64_STANDARD
            .decode(payload)
            .map_err(|_| bad_request("invalid payload"))?
            .into(),
        None => return Err(bad_request("missing payload")),
    };
    let retry = params
        .get("retry")
        .map(|retry| u64::from_str(retry))
        .transpose()
        .map_err(|_| bad_request("invalid retry value"))?;

    let handshake_frame = HandshakeRequestFrame::Handshake {
        service: id,
        pk,
        pop,
        retry,
    };

    let (frame_tx, frame_rx) = mpsc::channel(8);
    let (body_tx, body_rx) = mpsc::channel(1024);
    let (termination_tx, termination_rx) = oneshot::channel();

    let sender = HttpSender {
        frame_tx,
        body_tx,
        expected_body_len: 0,
        termination_tx: Some(termination_tx),
    };
    let receiver = HttpReceiver { inner: frame_rx };
    let body = StreamBody::new(ReceiverStream::new(body_rx));

    sender
        .frame_tx
        .try_send(Some(RequestFrame::ServicePayload { bytes: payload }))
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "unexpected error".to_string(),
            )
        })?;

    provider
        .handle_new_connection(handshake_frame, sender, receiver)
        .await;

    // If there is an error while streaming, the status header has already been sent,
    // this is a hacky way of returning an error status before beginning streaming the body.
    match termination_rx.await {
        Ok(reason) => Err(bad_request(format!("handshake failed: {reason:?}"))),
        Err(_) => Ok(body),
    }
}

pub struct HttpSender {
    frame_tx: Sender<Option<RequestFrame>>,
    body_tx: Sender<anyhow::Result<Bytes>>,
    expected_body_len: usize,
    termination_tx: Option<oneshot::Sender<TerminationReason>>,
}

impl HttpSender {
    #[inline(always)]
    fn close(&mut self) {
        if self.frame_tx.try_send(None).is_err() {
            warn!("failed to send close signal to receiver");
        }
    }

    #[inline(always)]
    fn inner_send(&mut self, bytes: Bytes) {
        if let Err(e) = self.body_tx.try_send(Ok(bytes)) {
            warn!("payload dropped, failed to send to write loop: {e}");
        }
    }
}

#[async_trait]
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
        {
            assert!(self.termination_tx.take().is_some());
        }

        self.expected_body_len = len;
        debug_assert!(
            self.expected_body_len == 0,
            "data should be written completely before calling start_write again"
        );
    }

    fn write(&mut self, buf: Bytes) -> anyhow::Result<usize> {
        let len = buf.len();

        debug_assert!(self.expected_body_len != 0);
        debug_assert!(self.expected_body_len >= len);

        self.inner_send(buf);

        if self.expected_body_len <= len {
            self.close();
        }

        self.expected_body_len -= len;

        Ok(len)
    }
}

pub struct HttpReceiver {
    inner: Receiver<Option<RequestFrame>>,
}

#[async_trait]
impl TransportReceiver for HttpReceiver {
    async fn recv(&mut self) -> Option<RequestFrame> {
        self.inner.recv().await.flatten()
    }
}

#[inline(always)]
fn bad_request<T: AsRef<str> + Display>(msg: T) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, msg.to_string())
}
