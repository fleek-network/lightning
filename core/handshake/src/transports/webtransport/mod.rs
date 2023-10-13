mod certificate;
mod config;
mod connection;

use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use axum::{Extension, Router};
pub use config::WebTransportConfig;
use fleek_crypto::{NodeSecretKey, SecretKey};
use futures::StreamExt;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tracing::error;
use wtransport::tls::Certificate;
use wtransport::{Endpoint, ServerConfig};

use crate::schema::{HandshakeRequestFrame, HandshakeResponse, RequestFrame, ResponseFrame};
use crate::shutdown::ShutdownWaiter;
use crate::transports::webtransport::connection::{Context, FramedStreamRx, FramedStreamTx};
use crate::transports::{Transport, TransportReceiver, TransportSender};

pub struct WebTransport {
    conn_rx: Receiver<(HandshakeRequestFrame, (FramedStreamTx, FramedStreamRx))>,
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
        let (data_tx, data_rx) = mpsc::channel(2048);
        tokio::spawn(connection::sender_loop(data_rx, frame_writer));
        Some((
            frame,
            WebTransportSender { tx: data_tx },
            WebTransportReceiver { rx: frame_reader },
        ))
    }
}

pub struct WebTransportSender {
    tx: Sender<Vec<u8>>,
}

macro_rules! webtransport_send {
    ($t1:expr, $t2:expr) => {
        let tx = $t1.tx.clone();
        let bytes = $t2.encode();
        tokio::spawn(async move {
            if let Err(e) = tx.send(bytes.to_vec()).await {
                error!("failed to send payload to connection loop: {e}");
            };
        });
    };
}

impl TransportSender for WebTransportSender {
    fn send_handshake_response(&mut self, response: HandshakeResponse) {
        webtransport_send!(self, response);
    }

    fn send(&mut self, frame: ResponseFrame) {
        webtransport_send!(self, frame);
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
            .with_certificate(Certificate::new(
                vec![cert_der],
                cert.serialize_private_key_der(),
            ))
            .keep_alive_interval(config.keep_alive)
            .build(),
    ))
}
