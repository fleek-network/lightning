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
use lightning_interfaces::prelude::*;
use lightning_metrics::increment_counter;
use tokio::sync::mpsc::{self, Receiver};
use tracing::{error, info};
use wtransport::tls::{Certificate, CertificateChain, PrivateKey};
use wtransport::{Endpoint, Identity, SendStream, ServerConfig};

use super::delimit_frame;
use crate::schema::{
    HandshakeRequestFrame,
    HandshakeResponse,
    RequestFrame,
    ResponseFrame,
    RES_SERVICE_PAYLOAD_TAG,
};
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
        info!("Binding WebTransport on {}", config.address);

        let (cert_hash, server_config) =
            create_cert_hash_and_server_config(NodeSecretKey::generate(), config.clone()).await?;

        let shared_cert_hash = Arc::new(RwLock::new(cert_hash));
        let router = Router::new()
            .route(
                "/certificate-hash",
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
        spawn!(
            connection::main_loop(ctx),
            "HANDSHAKE: webtransport main loop"
        );

        Ok((
            Self {
                conn_rx: conn_event_rx,
            },
            Some(router),
        ))
    }

    async fn accept(&mut self) -> Option<(HandshakeRequestFrame, Self::Sender, Self::Receiver)> {
        let (frame, (writer, frame_reader)) = self.conn_rx.recv().await?;

        increment_counter!(
            "handshake_webtransport_sessions",
            Some("Counter for number of handshake sessions accepted over webtransport")
        );

        Some((
            frame,
            WebTransportSender {
                writer,
                current_write: 0,
            },
            WebTransportReceiver { rx: frame_reader },
        ))
    }
}

pub struct WebTransportSender {
    writer: SendStream,
    current_write: u32,
}

impl WebTransportSender {
    #[inline(always)]
    async fn send_inner(&mut self, buf: &[u8]) {
        if let Err(e) = self.writer.write_all(buf).await {
            error!("failed to send data: {e:?}");
        }
    }
}

impl TransportSender for WebTransportSender {
    #[inline(always)]
    async fn send_handshake_response(&mut self, response: HandshakeResponse) {
        self.send_inner(&delimit_frame(response.encode())).await;
    }

    #[inline(always)]
    async fn send(&mut self, frame: ResponseFrame) {
        debug_assert!(
            !matches!(
                frame,
                ResponseFrame::ServicePayload { .. } | ResponseFrame::ServicePayloadChunk { .. }
            ),
            "payloads should only be sent via start_write and write"
        );

        self.send_inner(&delimit_frame(frame.encode())).await;
    }

    #[inline(always)]
    async fn start_write(&mut self, len: usize) {
        let len = len as u32;
        debug_assert!(
            self.current_write == 0,
            "data should be written completely before calling start_write again"
        );

        self.current_write = len;

        let mut buffer = Vec::with_capacity(5);
        // add 1 to the delimiter to include the frame tag
        buffer.put_u32(len + 1);
        buffer.put_u8(RES_SERVICE_PAYLOAD_TAG);
        // write the delimiter and payload tag to the stream
        self.send_inner(&buffer).await;
    }

    #[inline(always)]
    async fn write(&mut self, buf: Bytes) -> anyhow::Result<usize> {
        let len = u32::try_from(buf.len())?;
        debug_assert!(self.current_write != 0);
        debug_assert!(self.current_write >= len);

        self.current_write -= len;
        self.send_inner(&buf).await;
        Ok(len as usize)
    }
}

pub struct WebTransportReceiver {
    rx: FramedStreamRx,
}

impl TransportReceiver for WebTransportReceiver {
    #[inline(always)]
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

pub async fn create_cert_hash_and_server_config(
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
            .with_identity(&Identity::new(
                CertificateChain::single(Certificate::from_der(cert_der)?),
                PrivateKey::from_der_pkcs8(cert.serialize_private_key_der()),
            ))
            .keep_alive_interval(config.keep_alive)
            .build(),
    ))
}
