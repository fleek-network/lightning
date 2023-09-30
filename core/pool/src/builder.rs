use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use fleek_crypto::NodeSecretKey;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::ApplicationInterface;
use quinn::{ServerConfig, TransportConfig};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::connection::ConnectionPool;
use crate::tls;

pub struct Builder<C: Collection> {
    sk: NodeSecretKey,
    transport_config: TransportConfig,
    address: Option<SocketAddr>,
    topology: c![C::TopologyInterface],
    sync_query: c![C::ApplicationInterface::SyncExecutor],
}

impl<C: Collection> Builder<C> {
    pub fn new(
        sk: NodeSecretKey,
        topology: c!(C::TopologyInterface),
        sync_query: c!(C::ApplicationInterface::SyncExecutor),
    ) -> Self {
        Self {
            sk,
            transport_config: TransportConfig::default(),
            address: None,
            topology,
            sync_query,
        }
    }

    pub fn socket_address(&mut self, address: SocketAddr) {
        self.address = Some(address);
    }

    pub fn keep_alive_interval(&mut self, duration: Duration) {
        self.transport_config.keep_alive_interval(Some(duration));
    }

    pub fn build<W, R>(self) -> Result<ConnectionPool<C, W, R>>
    where
        W: AsyncWrite + Send + 'static,
        R: AsyncRead + Send + 'static,
    {
        let tls_config = tls::make_server_config(&self.sk).expect("Secret key to be valid");
        let mut server_config = ServerConfig::with_crypto(Arc::new(tls_config));
        server_config.transport_config(Arc::new(self.transport_config));

        let address: SocketAddr = self
            .address
            .unwrap_or_else(|| "0.0.0.0:0".parse().expect("hardcoded IP address"));

        Ok(ConnectionPool::new(
            self.topology,
            self.sync_query,
            self.sk,
            server_config,
            address,
        ))
    }
}

impl<C: Collection> Clone for Builder<C> {
    fn clone(&self) -> Self {
        Self {
            sk: self.sk.clone(),
            transport_config: TransportConfig::default(),
            address: self.address,
            topology: self.topology.clone(),
            sync_query: self.sync_query.clone(),
        }
    }
}
