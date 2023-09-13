mod certificate;
mod config;
mod connection;

use async_trait::async_trait;
pub use config::WebTransportConfig;
use fleek_crypto::{NodeSecretKey, SecretKey};
use futures::StreamExt;
use tokio::sync::mpsc::{self, Receiver, Sender};
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

    async fn bind(shutdown: ShutdownWaiter, config: Self::Config) -> anyhow::Result<Self> {
        let (cert_der, pk) = match config.certificate {
            None => {
                log::warn!("no certificate found in config so generating one from random secret");
                let certificate = certificate::generate_certificate(NodeSecretKey::generate())?;
                (
                    certificate.serialize_der()?,
                    certificate.serialize_private_key_der(),
                )
            },
            Some(cert) => (cert.certificate, cert.key),
        };

        let config = ServerConfig::builder()
            .with_bind_address(config.address)
            .with_certificate(Certificate::new(vec![cert_der], pk))
            .keep_alive_interval(config.keep_alive)
            .build();

        let endpoint = Endpoint::server(config)?;
        let (conn_tx, conn_rx) = mpsc::channel(2048);
        let ctx = Context {
            endpoint,
            accept_tx: conn_tx,
            shutdown,
        };
        tokio::spawn(connection::main_loop(ctx));

        Ok(Self { conn_rx })
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

impl TransportSender for WebTransportSender {
    fn send_handshake_response(&mut self, response: HandshakeResponse) {
        let tx = self.tx.clone();
        let data = response.encode();
        tokio::spawn(async move {
            if tx.send(data.to_vec()).await.is_err() {
                log::error!("failed to send data to connection loop");
            }
        });
    }

    fn send(&mut self, frame: ResponseFrame) {
        let tx = self.tx.clone();
        let data = frame.encode();
        tokio::spawn(async move {
            if tx.send(data.to_vec()).await.is_err() {
                log::error!("failed to send data to connection loop");
            }
        });
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
                log::error!("failed to get next frame: {e:?}");
                return None;
            },
        };
        match RequestFrame::decode(&data) {
            Ok(data) => Some(data),
            Err(e) => {
                log::error!("failed to decode request frame: {e:?}");
                None
            },
        }
    }
}
