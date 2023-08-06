use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use fleek_crypto::NodeNetworkingPublicKey;
use lightning_application::query_runner::QueryRunner;
use lightning_interfaces::{
    dht::{DhtInterface, TableEntry, TablePrefix},
    SignerInterface, WithStartAndShutdown,
};
use lightning_topology::Topology;
use tokio::{
    net::UdpSocket,
    sync::{mpsc, oneshot},
};

use crate::{bootstrap, bootstrap::Query, handler, handler::Command, query::NodeInfo, table};

/// Builds the DHT.
#[derive(Default)]
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

    /// Set address to bind the node's socket to.
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

/// Maintains the DHT.
pub struct Dht {
    handler_tx: mpsc::Sender<Command>,
    bootstrap_tx: mpsc::Sender<Query>,
}

impl Dht {
    /// Return one value associated with the given key.
    pub async fn get(&self, key: &[u8]) -> Option<TableEntry> {
        let (tx, rx) = oneshot::channel();
        if self
            .handler_tx
            .send(Command::Get {
                key: key.to_vec(),
                tx,
            })
            .await
            .is_err()
        {
            tracing::error!("failed to send to handler task");
        }
        match rx.await {
            Ok(value) => value.unwrap_or_else(|_| {
                tracing::trace!("unexpected error when attempting to get {key:?}");
                None
            }),
            Err(_) => {
                tracing::error!("sender was dropped unexpectedly");
                None
            },
        }
    }

    /// Put a key-value pair into the DHT.
    pub fn put(&self, key: &[u8], value: &[u8]) {
        // Todo: Maybe we should make `put` async.
        futures::executor::block_on(async {
            if self
                .handler_tx
                .send(Command::Put {
                    key: key.to_vec(),
                    value: value.to_vec(),
                })
                .await
                .is_err()
            {
                tracing::error!("failed to send to handler task");
            }
        });
    }

    // Todo: Let's return a proper error to users.
    /// Start bootstrap task.
    pub async fn bootstrap(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        if self.bootstrap_tx.send(Query::Start { tx }).await.is_err() {
            tracing::error!("failed to send to bootstrap task");
        }
        if rx.await.unwrap_or(false) {
            tracing::warn!("failed to start bootstrap");
        }
        Ok(())
    }

    /// Returns true if the node is bootstrapped and false otherwise.
    pub async fn is_bootstrapped(&self) -> bool {
        let (tx, rx) = oneshot::channel();
        if self
            .bootstrap_tx
            .send(Query::DoneBootstrapping { tx })
            .await
            .is_err()
        {
            tracing::error!("failed to send to bootstrap task");
        }
        rx.await.unwrap_or(false)
    }
}

#[async_trait]
impl WithStartAndShutdown for Dht {
    fn is_running(&self) -> bool {
        todo!()
    }

    async fn start(&self) {
        todo!()
    }

    async fn shutdown(&self) {
        todo!()
    }
}

// Todo: Rename interface.
#[async_trait]
impl DhtInterface for Dht {
    type Topology = Topology<QueryRunner>;

    async fn init<S: SignerInterface>(_: &S, _: Arc<Self::Topology>) -> Result<Self> {
        Builder::default().build().await
    }

    fn put(&self, _: TablePrefix, key: &[u8], value: &[u8]) {
        self.put(key, value)
    }

    async fn get(&self, _: TablePrefix, key: &[u8]) -> Option<TableEntry> {
        self.get(key).await
    }
}
