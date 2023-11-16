use std::net::SocketAddr;

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use tokio::io::AsyncReadExt;
use wtransport::endpoint::endpoint_side::Client;
use wtransport::{ClientConfig, Connection, Endpoint};

use crate::tls;
use crate::transport::{Transport, TransportStream};

pub struct WebTransport {
    target: String,
    endpoint: Endpoint<Client>,
}

impl WebTransport {
    pub fn new(config: Config) -> Result<Self> {
        let client_config = ClientConfig::builder()
            .with_bind_address(config.bind_address)
            // Todo: We need to customize handshake to validate server certificate hashes.
            // Maybe we don't have to do this if we perform authentication using keys
            // in a custom TLS validator similarly to libp2p.
            .with_custom_tls(tls::tls_config(config.server_hashes))
            .build();
        let endpoint = Endpoint::client(client_config)?;
        Ok(Self {
            endpoint,
            target: config.target,
        })
    }
}

pub struct Config {
    pub target: String,
    pub server_hashes: Vec<Vec<u8>>,
    pub bind_address: SocketAddr,
}

#[async_trait]
impl Transport for WebTransport {
    type Stream = WebTransportStream;

    async fn connect(&self) -> anyhow::Result<Self::Stream> {
        let conn = self.endpoint.connect(&self.target).await?;
        Ok(WebTransportStream { inner: conn })
    }
}

pub struct WebTransportStream {
    inner: Connection,
}

#[async_trait]
impl TransportStream for WebTransportStream {
    async fn send(&self, data: &[u8]) -> Result<()> {
        let mut writer = self.inner.open_uni().await?.await?;
        writer.write_all(data).await.map_err(Into::into)
    }

    async fn recv(&mut self) -> Result<Bytes> {
        let mut receiver = self.inner.accept_uni().await?;
        // Todo: verify that read_to_end is cancel safe.
        let mut buffer = Vec::new();
        receiver.read_to_end(buffer.as_mut()).await?;
        Ok(Bytes::from(buffer))
    }
}
