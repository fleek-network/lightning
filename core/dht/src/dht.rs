use std::{
    marker::PhantomData,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use affair::{Socket, Task};
use anyhow::Result;
use async_trait::async_trait;
use fleek_crypto::{NodeNetworkingPublicKey, NodeNetworkingSecretKey, SecretKey};
use lightning_interfaces::{
    dht::{DhtInterface, DhtSocket},
    types::{DhtResponse, TableEntry},
    ConfigConsumer, SignerInterface, TopologyInterface, WithStartAndShutdown,
};
use tokio::{
    net::UdpSocket,
    sync::{mpsc, oneshot, Notify},
};

use crate::{
    api,
    api::DhtRequest,
    bootstrap,
    bootstrap::BootstrapRequest,
    config::Config,
    query,
    query::{HandlerRequest, NodeInfo},
    store, table, task,
};

/// Builds the DHT.
pub struct Builder {
    config: Config,
    nodes: Vec<NodeInfo>,
    network_secret_key: NodeNetworkingSecretKey,
    buffer_size: Option<usize>,
}

impl Builder {
    /// Returns a new [`Builder`].
    pub fn new(network_secret_key: NodeNetworkingSecretKey, config: Config) -> Self {
        let nodes: Vec<NodeInfo> = config
            .bootstrappers
            .iter()
            .map(|b| NodeInfo {
                key: b.network_public_key,
                address: b.address,
            })
            .collect();
        Self {
            config,
            nodes,
            network_secret_key,
            buffer_size: None,
        }
    }

    /// Add node which will be added to routing table.
    pub fn add_node(&mut self, key: NodeNetworkingPublicKey, address: SocketAddr) {
        self.nodes.push(NodeInfo { key, address });
    }

    /// Set buffer size for tasks.
    pub fn set_buffer_size(&mut self, size: usize) {
        self.buffer_size = Some(size);
    }

    /// Build and initiates the DHT.
    pub fn build<T: TopologyInterface>(self) -> Result<Dht<T>> {
        let buffer_size = self.buffer_size.unwrap_or(10_000);

        let (socket, rx) = Socket::raw_bounded(2048);
        let (handler_tx, handler_rx) = mpsc::channel(buffer_size);

        Ok(Dht {
            socket,
            nodes: Arc::new(Mutex::new(Some(self.nodes))),
            buffer_size,
            address: self.config.address,
            network_secret_key: self.network_secret_key,
            handler_tx,
            handler_rx: Arc::new(Mutex::new(Some(handler_rx))),
            bootstrap_notify: Arc::new(Notify::new()),
            is_running: Arc::new(Mutex::new(false)),
            shutdown_notify: Arc::new(Notify::new()),
            topology: PhantomData,
        })
    }
}

/// Maintains the DHT.
#[allow(clippy::type_complexity)]
pub struct Dht<T: TopologyInterface> {
    socket: DhtSocket,
    buffer_size: usize,
    address: SocketAddr,
    network_secret_key: NodeNetworkingSecretKey,
    handler_tx: mpsc::Sender<DhtRequest>,
    handler_rx: Arc<Mutex<Option<mpsc::Receiver<DhtRequest>>>>,
    bootstrap_notify: Arc<Notify>,
    nodes: Arc<Mutex<Option<Vec<NodeInfo>>>>,
    is_running: Arc<Mutex<bool>>,
    shutdown_notify: Arc<Notify>,
    topology: PhantomData<T>,
}

impl<T: TopologyInterface> Dht<T> {
    /// Return one value associated with the given key.
    pub async fn get(handler_tx: mpsc::Sender<HandlerRequest>, key: &[u8]) -> Option<TableEntry> {
        let (tx, rx) = oneshot::channel();
        if handler_tx
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
    pub fn put(handler_tx: mpsc::Sender<HandlerRequest>, key: &[u8], value: &[u8]) {
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
        self.bootstrap_notify.notify_waiters();
    }
}

#[async_trait]
impl<T: TopologyInterface> WithStartAndShutdown for Dht<T> {
    fn is_running(&self) -> bool {
        *self.is_running.lock().unwrap()
    }

    async fn start(&self) {
        let public_key = self.network_secret_key.to_pk();
        let (table_tx, table_rx) = mpsc::channel(self.buffer_size);

        tokio::spawn(table::start_worker(
            table_rx,
            public_key,
            self.shutdown_notify.clone(),
        ));

        let (store_tx, store_rx) = mpsc::channel(self.buffer_size);
        tokio::spawn(store::start_worker(store_rx, self.shutdown_notify.clone()));

        let socket = UdpSocket::bind(self.address)
            .await
            .map(Arc::new)
            .expect("Binding to socket failed");
        tracing::info!("UDP socket bound to {:?}", socket.local_addr().unwrap());

        let (task_tx, task_rx) = mpsc::channel(self.buffer_size);
        tokio::spawn(api::start_worker(
            self.handler_rx.lock().unwrap().take().unwrap(),
            task_tx,
            self.bootstrap_notify.clone(),
            self.shutdown_notify.clone(),
            self.network_secret_key.to_pk(),
            socket.clone(),
        ));

        let (event_queue_tx, event_queue_rx) = mpsc::channel(self.buffer_size);
        tokio::spawn(task::start_worker(
            task_rx,
            event_queue_rx,
            table_tx.clone(),
            socket.clone(),
        ));

        tokio::spawn(query::start_worker(
            event_queue_tx,
            table_tx,
            store_tx,
            socket,
            public_key,
            self.shutdown_notify.clone(),
        ));

        // Todo: Check that it is done bootstrapping.
        self.bootstrap().await;

        *self.is_running.lock().unwrap() = true;
    }

    async fn shutdown(&self) {
        self.shutdown_notify.notify_waiters();
        *self.is_running.lock().unwrap() = false;
    }
}

#[async_trait]
impl<T: TopologyInterface> DhtInterface for Dht<T> {
    type Topology = T;

    fn init<S: SignerInterface>(
        signer: &S,
        _: Arc<Self::Topology>,
        config: Self::Config,
    ) -> Result<Self> {
        let (network_secret_key, _) = signer.get_sk();
        Builder::new(network_secret_key, config).build()
    }

    fn get_socket(&self) -> DhtSocket {
        self.socket.clone()
    }
}

impl<T: TopologyInterface> ConfigConsumer for Dht<T> {
    const KEY: &'static str = "dht";

    type Config = Config;
}
