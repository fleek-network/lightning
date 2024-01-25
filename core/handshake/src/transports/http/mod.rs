mod config;

use std::collections::HashMap;
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
};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_stream::wrappers::ReceiverStream;

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

impl HttpTransport {
    fn create_pair_and_body() -> (
        <Self as Transport>::Sender,
        <Self as Transport>::Receiver,
        StreamBody<ReceiverStream<anyhow::Result<Bytes>>>,
    ) {
        let (receiver_tx, receiver_rx) = mpsc::channel(4);
        let (body_tx, body_rx) = mpsc::channel(4);
        let sender = HttpSender {
            receiver_tx,
            body_tx,
        };
        let body = StreamBody::new(ReceiverStream::new(body_rx));
        (sender, HttpReceiver { inner: receiver_rx }, body)
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
                .map_err(|_| (StatusCode::BAD_REQUEST, "invalid pk value".to_string()))?
                .try_into()
                .map_err(|_| (StatusCode::BAD_REQUEST, "invalid pk value".to_string()))?;
            ClientPublicKey(bytes)
        },
        None => return Err((StatusCode::BAD_REQUEST, "missing pk value".to_string())),
    };
    let pop = match params.get("pop") {
        Some(pk) => {
            let bytes: [u8; 48] = base64::prelude::BASE64_STANDARD
                .decode(pk)
                .map_err(|_| (StatusCode::BAD_REQUEST, "invalid pop value".to_string()))?
                .try_into()
                .map_err(|_| (StatusCode::BAD_REQUEST, "invalid pop value".to_string()))?;
            ClientSignature(bytes)
        },
        None => return Err((StatusCode::BAD_REQUEST, "missing pop value".to_string())),
    };
    let retry = params
        .get("retry")
        .and_then(|retry| Some(u64::from_str(retry)))
        .transpose()
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid retry value".to_string()))?;

    let handshake_frame = HandshakeRequestFrame::Handshake {
        service: id,
        pk,
        pop,
        retry,
    };

    let (sender, receiver, body) = HttpTransport::create_pair_and_body();

    provider
        .handle_new_connection(handshake_frame, sender, receiver)
        .await;

    Ok(body)
}

pub struct HttpSender {
    receiver_tx: Sender<RequestFrame>,
    body_tx: Sender<anyhow::Result<Bytes>>,
}

#[async_trait]
impl TransportSender for HttpSender {
    fn send_handshake_response(&mut self, response: HandshakeResponse) {
        todo!()
    }

    fn send(&mut self, frame: ResponseFrame) {
        todo!()
    }

    fn start_write(&mut self, len: usize) {
        todo!()
    }

    fn write(&mut self, buf: &[u8]) -> anyhow::Result<usize> {
        todo!()
    }
}

pub struct HttpReceiver {
    inner: Receiver<RequestFrame>,
}

#[async_trait]
impl TransportReceiver for HttpReceiver {
    async fn recv(&mut self) -> Option<RequestFrame> {
        todo!()
    }
}
