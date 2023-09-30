use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use fleek_crypto::NodeSecretKey;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::{ApplicationInterface, NotifierInterface};
use quinn::{ServerConfig, TransportConfig, VarInt};
use tokio::sync::mpsc;

use crate::endpoint::Endpoint;
use crate::tls;

pub struct Builder<C: Collection> {
    sk: NodeSecretKey,
    transport_config: TransportConfig,
    address: Option<SocketAddr>,
    topology: c![C::TopologyInterface],
    sync_query: c![C::ApplicationInterface::SyncExecutor],
    notifier: c![C::NotifierInterface],
}

impl<C: Collection> Builder<C> {
    pub fn new(
        sk: NodeSecretKey,
        topology: c!(C::TopologyInterface),
        sync_query: c!(C::ApplicationInterface::SyncExecutor),
        notifier: c!(C::NotifierInterface),
    ) -> Self {
        Self {
            sk,
            transport_config: TransportConfig::default(),
            address: None,
            topology,
            sync_query,
            notifier,
        }
    }

    pub fn socket_address(&mut self, address: SocketAddr) {
        self.address = Some(address);
    }

    pub fn max_idle_timeout(&mut self, timeout: u32) {
        self.transport_config
            .max_idle_timeout(Some(VarInt::from_u32(timeout).into()));
    }

    pub fn build(self) -> Result<Endpoint<C>> {
        let tls_config = tls::make_server_config(&self.sk).expect("Secret key to be valid");
        let mut server_config = ServerConfig::with_crypto(Arc::new(tls_config));
        server_config.transport_config(Arc::new(self.transport_config));

        let (notifier_tx, notifier_rx) = mpsc::channel(32);
        self.notifier.notify_on_new_epoch(notifier_tx);

        let address: SocketAddr = self
            .address
            .unwrap_or_else(|| "0.0.0.0:0".parse().expect("hardcoded IP address"));

        Ok(Endpoint::new(
            self.topology,
            self.sync_query,
            notifier_rx,
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
            notifier: self.notifier.clone(),
        }
    }
}
