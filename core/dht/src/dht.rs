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
    types::{DhtRequest, DhtResponse, TableEntry},
    SignerInterface, TopologyInterface, WithStartAndShutdown,
};
use tokio::{
    net::UdpSocket,
    sync::{mpsc, oneshot, Notify},
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
    pub fn build<T: TopologyInterface>(self) -> Result<Dht<T>> {
        let buffer_size = self.buffer_size.unwrap_or(10_000);
        let address = self.address.unwrap_or_else(|| "0.0.0.0:0".parse().unwrap());

        let (socket, rx) = Socket::raw_bounded(2048);
        let (handler_tx, handler_rx) = mpsc::channel(buffer_size);
        let (bootstrap_tx, bootstrap_rx) = mpsc::channel(buffer_size);

        Ok(Dht {
            socket,
            socket_rx: Arc::new(Mutex::new(Some(rx))),
            nodes: Arc::new(Mutex::new(Some(self.nodes))),
            buffer_size,
            address,
            network_secret_key: self.network_secret_key,
            handler_tx,
            bootstrap_tx,
            handler_rx: Arc::new(Mutex::new(Some(handler_rx))),
            bootstrap_rx: Arc::new(Mutex::new(Some(bootstrap_rx))),
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
    socket_rx: Arc<Mutex<Option<mpsc::Receiver<Task<DhtRequest, DhtResponse>>>>>,
    buffer_size: usize,
    address: SocketAddr,
    network_secret_key: NodeNetworkingSecretKey,
    handler_tx: mpsc::Sender<HandlerRequest>,
    bootstrap_tx: mpsc::Sender<BootstrapRequest>,
    handler_rx: Arc<Mutex<Option<mpsc::Receiver<HandlerRequest>>>>,
    bootstrap_rx: Arc<Mutex<Option<mpsc::Receiver<BootstrapRequest>>>>,
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
        if self
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

        let (worker_tx, worker_rx) = mpsc::channel(self.buffer_size);
        tokio::spawn(store::start_worker(worker_rx, self.shutdown_notify.clone()));

        let socket = UdpSocket::bind(self.address)
            .await
            .map(Arc::new)
            .expect("Binding to socket failed");
        tracing::info!("UDP socket bound to {:?}", socket.local_addr().unwrap());

        tokio::spawn(handler::start_worker(
            self.handler_rx.lock().unwrap().take().unwrap(),
            table_tx.clone(),
            worker_tx,
            socket,
            self.network_secret_key.to_pk(),
            self.shutdown_notify.clone(),
        ));
        tokio::spawn(bootstrap::start_worker(
            self.bootstrap_rx.lock().unwrap().take().unwrap(),
            table_tx,
            self.handler_tx.clone(),
            self.network_secret_key.to_pk(),
            self.nodes.lock().unwrap().take().unwrap(),
        ));

        self.bootstrap().await;
        while !self.is_bootstrapped().await {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        // TODO: this will be refactored soon
        let mut socket_rx = self.socket_rx.lock().unwrap().take().unwrap();
        let shutdown_notify = self.shutdown_notify.clone();
        let handler_tx = self.handler_tx.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    task = socket_rx.recv() => {
                        if let Some(task) = task {
                            match task.request.clone() {
                                DhtRequest::Get { prefix: _, key } => {
                                    let entry = Dht::<T>::get(handler_tx.clone(), &key).await;
                                    task.respond(DhtResponse::Get(entry));
                                }
                                DhtRequest::Put { prefix: _, key, value } => {
                                    Dht::<T>::put(handler_tx.clone(), &key, &value);
                                    task.respond(DhtResponse::Put(()));
                                }
                            }
                        }

                    }
                    _ = shutdown_notify.notified() => {
                        break;
                    }
                }
            }
        });

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

    fn init<S: SignerInterface>(signer: &S, _: Arc<Self::Topology>) -> Result<Self> {
        let (network_secret_key, _) = signer.get_sk();
        Builder::new(network_secret_key).build()
    }

    fn get_socket(&self) -> DhtSocket {
        self.socket.clone()
    }
}
