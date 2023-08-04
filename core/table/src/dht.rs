use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use fleek_crypto::NodeNetworkingPublicKey;
use tokio::{net::UdpSocket, sync::mpsc};

use crate::{bootstrap, bootstrap::Query, handler, handler::Command, query::NodeInfo, table};

/// Builds the DHT.
pub struct Builder {
    nodes: Vec<NodeInfo>,
    node_key: Option<NodeNetworkingPublicKey>,
    address: Option<SocketAddr>,
    buffer_size: Option<usize>,
}

impl Builder {
    /// Add node which will be added to routing table.
    pub fn add_node(&mut self, node: NodeInfo) {
        self.nodes.push(node);
    }

    /// Set key of this node.
    pub fn set_node_key(&mut self, key: NodeNetworkingPublicKey) {
        self.node_key = Some(key);
    }

    /// Set address to bind to.
    pub fn set_address(&mut self, address: SocketAddr) {
        self.address = Some(address);
    }

    /// Set buffer size for tasks.
    pub fn set_buffer_size(&mut self, size: usize) {
        self.buffer_size = Some(size);
    }

    /// Build and initiates the DHT.
    pub async fn build(self) -> Result<Dht> {
        let buffer_size = self.buffer_size.unwrap_or(10_000);
        let node_key = self.node_key.unwrap_or_else(|| {
            tracing::warn!("generating random key");
            NodeNetworkingPublicKey(rand::random())
        });
        let (table_tx, table_rx) = mpsc::channel(buffer_size);
        tokio::spawn(table::start_server(table_rx, node_key));

        let address = self
            .address
            .unwrap_or_else(|| "0.0.0.0:0".parse().expect("Hardcoded address to be valid"));
        let socket = UdpSocket::bind(address).await.map(Arc::new)?;
        let (handler_tx, handler_rx) = mpsc::channel(buffer_size);
        tokio::spawn(handler::start_server(
            handler_rx,
            table_tx.clone(),
            socket,
            node_key,
        ));

        let (bootstrap_tx, bootstrap_rx) = mpsc::channel(buffer_size);
        tokio::spawn(bootstrap::start_server(
            bootstrap_rx,
            table_tx,
            handler_tx.clone(),
            node_key,
            self.nodes,
        ));

        Ok(Dht {
            handler_tx,
            bootstrap_tx,
        })
    }
}

pub struct Dht {
    handler_tx: mpsc::Sender<Command>,
    bootstrap_tx: mpsc::Sender<Query>,
}
