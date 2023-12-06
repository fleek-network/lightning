mod certificate;
mod config;
mod connection;

use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use axum::{Extension, Router};
use bytes::{BufMut as _, Bytes};
pub use config::WebTransportConfig;
use fleek_crypto::{NodeSecretKey, SecretKey};
use futures::StreamExt;
use tokio::sync::mpsc::{self, Receiver};
use tracing::{error, warn};
use wtransport::tls::Certificate;
use wtransport::{Endpoint, SendStream, ServerConfig};

use super::delimit_frame;
use crate::schema::{
    HandshakeRequestFrame,
    HandshakeResponse,
    RequestFrame,
    ResponseFrame,
    RES_SERVICE_PAYLOAD_TAG,
};
use crate::shutdown::ShutdownWaiter;
use crate::transports::webtransport::connection::{Context, FramedStreamRx};
use crate::transports::{Transport, TransportReceiver, TransportSender};

pub struct WebTransport {
    conn_rx: Receiver<(HandshakeRequestFrame, (SendStream, FramedStreamRx))>,
}

#[async_trait]
impl Transport for WebTransport {
    type Config = WebTransportConfig;
    type Sender = WebTransportSender;
    type Receiver = WebTransportReceiver;

    async fn bind(
        shutdown: ShutdownWaiter,
        config: Self::Config,
    ) -> anyhow::Result<(Self, Option<Router>)> {
        let (cert_hash, server_config) =
            create_cert_hash_and_server_config(NodeSecretKey::generate(), config.clone())?;

        let shared_cert_hash = Arc::new(RwLock::new(cert_hash));
        let router = Router::new()
            .route(
                "certificate-hash",
                axum::routing::get(
                    |Extension(cert_hash): Extension<Arc<RwLock<Vec<u8>>>>| async move {
                        cert_hash.read().unwrap().to_vec()
                    },
                ),
            )
            .layer(Extension(shared_cert_hash.clone()));

        let endpoint = Endpoint::server(server_config)?;

        let (conn_event_tx, conn_event_rx) = mpsc::channel(2048);

        let ctx = Context {
            endpoint,
            accept_tx: conn_event_tx,
            published_cert_hash: shared_cert_hash,
            transport_config: config,
            shutdown,
        };
        tokio::spawn(connection::main_loop(ctx));

        Ok((
            Self {
                conn_rx: conn_event_rx,
            },
            Some(router),
        ))
    }

    async fn accept(&mut self) -> Option<(HandshakeRequestFrame, Self::Sender, Self::Receiver)> {
        let (frame, (frame_writer, frame_reader)) = self.conn_rx.recv().await?;
        let (data_tx, data_rx) = async_channel::unbounded();
        tokio::spawn(connection::sender_loop(data_rx, frame_writer));
        Some((
            frame,
            WebTransportSender {
                tx: data_tx,
                current_write: 0,
            },
            WebTransportReceiver { rx: frame_reader },
        ))
    }
}

pub struct WebTransportSender {
    tx: async_channel::Sender<Bytes>,
    current_write: u32,
}

impl WebTransportSender {
    #[inline(always)]
    fn send_inner(&mut self, bytes: Bytes) {
        if let Err(e) = self.tx.try_send(bytes) {
            warn!("payload dropped, failed to send to write loop: {e}");
        }
    }
}

impl TransportSender for WebTransportSender {
    fn send_handshake_response(&mut self, response: HandshakeResponse) {
        self.send_inner(delimit_frame(response.encode()));
    }

    fn send(&mut self, frame: ResponseFrame) {
        debug_assert!(
            !matches!(
                frame,
                ResponseFrame::ServicePayload { .. } | ResponseFrame::ServicePayloadChunk { .. }
            ),
            "payloads should only be sent via start_write and write"
        );

        self.send_inner(delimit_frame(frame.encode()));
    }

    fn start_write(&mut self, len: usize) {
        debug_assert!(
            self.current_write == 0,
            "data should be written completely before calling start_write again"
        );

        self.current_write = len as u32;

        // add 1 to the length to include the frame tag
        let len = len as u32 + 1;
        let mut buffer = Vec::with_capacity(5);
        buffer.put_u32(len);
        buffer.put_u8(RES_SERVICE_PAYLOAD_TAG);
        // write the delimiter and payload tag to the stream
        self.send_inner(buffer.into());

        self.current_write = len;
    }

    fn write(&mut self, buf: &[u8]) -> anyhow::Result<usize> {
        let len = buf.len() as u32;
        debug_assert!(self.current_write != 0);
        debug_assert!(self.current_write >= len);

        self.current_write -= len;
        self.send_inner(buf.to_vec().into());
        Ok(buf.len())
    }
}

pub struct WebTransportReceiver {
    rx: FramedStreamRx,
}

#[async_trait]
impl TransportReceiver for WebTransportReceiver {
    async fn recv(&mut self) -> Option<RequestFrame> {
        let data = match self.rx.next().await? {
            Ok(data) => data,
            Err(e) => {
                error!("failed to get next frame: {e:?}");
                return None;
            },
        };
        match RequestFrame::decode(&data) {
            Ok(data) => Some(data),
            Err(e) => {
                error!("failed to decode request frame: {e:?}");
                None
            },
        }
    }
}

pub fn create_cert_hash_and_server_config(
    sk: NodeSecretKey,
    config: WebTransportConfig,
) -> anyhow::Result<(Vec<u8>, ServerConfig)> {
    let cert = certificate::generate_certificate(sk)?;
    let cert_der = cert.serialize_der()?;

    let cert_hash = ring::digest::digest(&ring::digest::SHA256, &cert_der)
        .as_ref()
        .to_vec();

    Ok((
        cert_hash,
        ServerConfig::builder()
            .with_bind_address(config.address)
            .with_certificate(
                Certificate::new(vec![cert_der], cert.serialize_private_key_der())
                    .expect("failed to create serialized certificate"),
            )
            .keep_alive_interval(config.keep_alive)
            .build(),
    ))
}
