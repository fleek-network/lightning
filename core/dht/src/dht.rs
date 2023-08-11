use std::{
    marker::PhantomData,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use async_trait::async_trait;
use fleek_crypto::{NodeNetworkingPublicKey, NodeNetworkingSecretKey, SecretKey};
use lightning_interfaces::{
    dht::{DhtInterface, KeyPrefix, TableEntry},
    SignerInterface, TopologyInterface, WithStartAndShutdown,
};
use tokio::{
    net::UdpSocket,
    sync::{mpsc, oneshot},
};

use crate::{
    bootstrap, bootstrap::BootstrapRequest, handler, handler::HandlerRequest, query::NodeInfo,
    store, table,
};

/// Builds the DHT.
pub struct Builder {
    nodes: Vec<NodeInfo>,
    network_secret_key: NodeNetworkingSecretKey,
    address: Option<SocketAddr>,
    buffer_size: Option<usize>,
}

impl Builder {
    /// Returns a new [`Builder`].
    pub fn new(network_secret_key: NodeNetworkingSecretKey) -> Self {
        Self {
            nodes: vec![],
            network_secret_key,
            address: None,
            buffer_size: None,
        }
    }

    /// Add node which will be added to routing table.
    pub fn add_node(&mut self, key: NodeNetworkingPublicKey, address: SocketAddr) {
        self.nodes.push(NodeInfo { key, address });
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
    pub async fn build<T: TopologyInterface>(self) -> Result<Dht<T>> {
        let buffer_size = self.buffer_size.unwrap_or(10_000);
        let address = self.address.unwrap_or_else(|| "0.0.0.0:0".parse().unwrap());

        let (handler_tx, handler_rx) = mpsc::channel(buffer_size);

        let (bootstrap_tx, bootstrap_rx) = mpsc::channel(buffer_size);

        let signer = InnerDht {
            nodes: Arc::new(Mutex::new(Some(self.nodes))),
            buffer_size,
            address,
            network_secret_key: self.network_secret_key,
            handler_tx,
            bootstrap_tx,
            handler_rx: Arc::new(Mutex::new(Some(handler_rx))),
            bootstrap_rx: Arc::new(Mutex::new(Some(bootstrap_rx))),
            topology: PhantomData,
        };

        Ok(Dht {
            inner: Arc::new(signer),
        })
    }
}

/// Maintains the DHT.
#[derive(Clone)]
pub struct Dht<T: TopologyInterface> {
    inner: Arc<InnerDht<T>>,
}

struct InnerDht<T: TopologyInterface> {
    buffer_size: usize,
    address: SocketAddr,
    network_secret_key: NodeNetworkingSecretKey,
    handler_tx: mpsc::Sender<HandlerRequest>,
    bootstrap_tx: mpsc::Sender<BootstrapRequest>,
    handler_rx: Arc<Mutex<Option<mpsc::Receiver<HandlerRequest>>>>,
    bootstrap_rx: Arc<Mutex<Option<mpsc::Receiver<BootstrapRequest>>>>,
    nodes: Arc<Mutex<Option<Vec<NodeInfo>>>>,
    topology: PhantomData<T>,
}

impl<T: TopologyInterface> Dht<T> {
    /// Return one value associated with the given key.
    pub async fn get(&self, key: &[u8]) -> Option<TableEntry> {
        let (tx, rx) = oneshot::channel();
        if self
            .inner
            .handler_tx
            .send(HandlerRequest::Get {
                key: key.to_vec(),
                tx,
            })
            .await
            .is_err()
        {
            tracing::error!("failed to send to handler request");
        }
        rx.await
            .expect("handler worker to not drop the channel")
            .unwrap_or_else(|e| {
                tracing::trace!("unexpected error when attempting to get {key:?}: {e:?}");
                None
            })
    }

    /// Put a key-value pair into the DHT.
    pub fn put(&self, key: &[u8], value: &[u8]) {
        let handler_tx = self.inner.handler_tx.clone();
        let key = key.to_vec();
        let value = value.to_vec();
        tokio::spawn(async move {
            if handler_tx
                .send(HandlerRequest::Put { key, value })
                .await
                .is_err()
            {
                tracing::error!("failed to send to handler request");
            }
        });
    }

    /// Start bootstrap task.
    /// If bootstrapping is in process, this request will be ignored.
    pub async fn bootstrap(&self) {
        if self
            .inner
            .bootstrap_tx
            .send(BootstrapRequest::Start)
            .await
            .is_err()
        {
            tracing::error!("failed to send to bootstrap request");
        }
    }

    /// Returns true if the node is bootstrapped and false otherwise.
    pub async fn is_bootstrapped(&self) -> bool {
        let (tx, rx) = oneshot::channel();
        if self
            .inner
            .bootstrap_tx
            .send(BootstrapRequest::DoneBootstrapping { tx })
            .await
            .is_err()
        {
            tracing::error!("failed to send to bootstrap request");
        }
        rx.await.unwrap_or(false)
    }
}

#[async_trait]
impl<T: TopologyInterface> WithStartAndShutdown for Dht<T> {
    fn is_running(&self) -> bool {
        !self.inner.handler_tx.is_closed() && !self.inner.bootstrap_tx.is_closed()
    }

    async fn start(&self) {
        let public_key = self.inner.network_secret_key.to_pk();
        let (table_tx, table_rx) = mpsc::channel(self.inner.buffer_size);
        tokio::spawn(table::start_worker(table_rx, public_key));

        let (worker_tx, worker_rx) = mpsc::channel(self.inner.buffer_size);
        tokio::spawn(store::start_worker(worker_rx));

        let socket = UdpSocket::bind(self.inner.address)
            .await
            .map(Arc::new)
            .expect("Binding to socket failed");
        tracing::info!("UDP socket bound to {:?}", socket.local_addr().unwrap());

        tokio::spawn(handler::start_worker(
            self.inner.handler_rx.lock().unwrap().take().unwrap(),
            table_tx.clone(),
            worker_tx,
            socket,
            self.inner.network_secret_key.to_pk(),
        ));
        tokio::spawn(bootstrap::start_worker(
            self.inner.bootstrap_rx.lock().unwrap().take().unwrap(),
            table_tx,
            self.inner.handler_tx.clone(),
            self.inner.network_secret_key.to_pk(),
            self.inner.nodes.lock().unwrap().take().unwrap(),
        ));
    }

    async fn shutdown(&self) {
        // We drop the boostrap worker first because
        // one of its tasks may be communicating with
        // the handler worker.
        self.inner
            .bootstrap_tx
            .send(BootstrapRequest::Shutdown)
            .await
            .expect("bootstrap worker to not drop channel");
        self.inner
            .handler_tx
            .send(HandlerRequest::Shutdown)
            .await
            .expect("handler worker to not drop channel");
    }
}

#[async_trait]
impl<T: TopologyInterface> DhtInterface for Dht<T> {
    type Topology = T;

    async fn init<S: SignerInterface>(signer: &S, _: Arc<Self::Topology>) -> Result<Self> {
        let (network_secret_key, _) = signer.get_sk();
        Builder::new(network_secret_key).build().await
    }

    fn put(&self, _: KeyPrefix, key: &[u8], value: &[u8]) {
        self.put(key, value)
    }

    async fn get(&self, _: KeyPrefix, key: &[u8]) -> Option<TableEntry> {
        self.get(key).await
    }
}
