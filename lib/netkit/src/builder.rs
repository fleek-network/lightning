use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use fleek_crypto::NodeSecretKey;
use quinn::{ServerConfig, TransportConfig};

use crate::endpoint::Endpoint;
use crate::tls;

pub struct Builder {
    sk: NodeSecretKey,
    transport_config: TransportConfig,
    address: Option<SocketAddr>,
}

impl Builder {
    pub fn new(sk: NodeSecretKey) -> Self {
        Self {
            sk,
            transport_config: TransportConfig::default(),
            address: None,
        }
    }

    pub fn socket_address(&mut self, address: SocketAddr) {
        self.address = Some(address);
    }

    pub fn transport_config(&mut self, config: TransportConfig) {
        self.transport_config = config;
    }

    pub fn build(mut self) -> Result<Endpoint> {
        let tls_config = tls::make_server_config(&self.sk).expect("Secret key to be valid");
        let mut server_config = ServerConfig::with_crypto(Arc::new(tls_config));
        self.transport_config
            .keep_alive_interval(Some(Duration::from_secs(5)));
        server_config.transport_config(Arc::new(self.transport_config));

        let address: SocketAddr = self
            .address
            .unwrap_or_else(|| "0.0.0.0:0".parse().expect("hardcoded IP address"));

        Ok(Endpoint::new(self.sk, address, server_config))
    }
}
