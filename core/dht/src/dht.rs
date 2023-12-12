use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use fleek_crypto::{NodePublicKey, NodeSecretKey, SecretKey};
use lightning_interfaces::dht::DhtInterface;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::types::{KeyPrefix, TableEntry};
use lightning_interfaces::{
    ApplicationInterface,
    ConfigConsumer,
    SignerInterface,
    SyncQueryRunnerInterface,
    WithStartAndShutdown,
};
use tokio::net::UdpSocket;
use tokio::sync::mpsc::Receiver;
use tokio::sync::{mpsc, Mutex, Notify};
use tokio::task::JoinHandle;

use crate::config::Config;
use crate::network::UdpTransport;
use crate::pool::{DhtLookUp, DhtPool, PoolWorker, Task};
use crate::table::worker::{Request, TableKey, TableWorker};
use crate::table::{DhtTable, Table, TableManager};

pub type TableWorkerType<C> = TableWorker<C, TableManager, DhtPool>;
pub type PoolWorkerType<C> = PoolWorker<
    DhtLookUp<C, DhtTable, UdpTransport<c![C::ApplicationInterface::SyncExecutor]>>,
    DhtTable,
    UdpTransport<c![C::ApplicationInterface::SyncExecutor]>,
>;

pub type LookUpType<C> =
    DhtLookUp<C, DhtTable, UdpTransport<c![C::ApplicationInterface::SyncExecutor]>>;

enum Status {
    NotRunning {
        table_request_receiver: Receiver<Request>,
        pool_task_receiver: Receiver<Task>,
    },
    Running {
        table_worker_handle: JoinHandle<Receiver<Request>>,
        pool_worker_handle: JoinHandle<Receiver<Task>>,
        shutdown: Arc<Notify>,
    },
}

/// Maintains the DHT.
#[allow(clippy::type_complexity, unused)]
pub struct Dht<C: Collection> {
    address: SocketAddr,
    table: DhtTable,
    pool: DhtPool,
    status: Arc<Mutex<Option<Status>>>,
    bootstrap_nodes: Vec<NodePublicKey>,
    sk: NodeSecretKey,
    sync_query: c![C::ApplicationInterface::SyncExecutor],
}
impl<C: Collection> Clone for Dht<C> {
    fn clone(&self) -> Self {
        Self {
            address: self.address,
            table: self.table.clone(),
            pool: self.pool.clone(),
            status: self.status.clone(),
            bootstrap_nodes: self.bootstrap_nodes.clone(),
            sk: self.sk.clone(),
            sync_query: self.sync_query.clone(),
        }
    }
}

#[async_trait]
impl<C> WithStartAndShutdown for Dht<C>
where
    C: Collection,
{
    fn is_running(&self) -> bool {
        let guard = self.status.blocking_lock();
        matches!(guard.as_ref().unwrap(), &Status::Running { .. })
    }

    async fn start(&self) {
        // Todo: Fix this because index may not be available.
        let local_pk = self.sk.to_pk();
        let local_index = self
            .sync_query
            .pubkey_to_index(local_pk)
            .unwrap_or(u32::MAX);
        let mut guard = self.status.lock().await;
        let mut status = guard.take().unwrap();

        match status {
            Status::NotRunning {
                table_request_receiver,
                pool_task_receiver,
            } => {
                let shutdown = Arc::new(Notify::new());
                // Todo: clean up socket in shutdown.
                let udp_socket = Arc::new(UdpSocket::bind(self.address).await.unwrap());
                let socket = UdpTransport::new(udp_socket, self.sync_query.clone());

                let looker: LookUpType<C> = DhtLookUp::new(
                    local_index,
                    self.table.clone(),
                    socket.clone(),
                    self.sync_query.clone(),
                );

                // Todo: make configurable.
                let max_pool_size = 2048;
                let (event_queue_tx, event_queue_rx) = mpsc::channel(1024);

                let pool_worker: PoolWorkerType<C> = PoolWorker::new(
                    local_index,
                    looker,
                    socket,
                    self.table.clone(),
                    pool_task_receiver,
                    event_queue_tx,
                    max_pool_size,
                    shutdown.clone(),
                );

                let table_worker: TableWorkerType<C> = TableWorker::new(
                    local_pk.0,
                    self.sync_query.clone(),
                    TableManager::new(local_pk),
                    self.pool.clone(),
                    table_request_receiver,
                    event_queue_rx,
                    shutdown.clone(),
                );

                // If it's empty, we're the bootstrap node.
                if !self.bootstrap_nodes.is_empty() {
                    if let Err(e) = self.table.bootstrap(self.bootstrap_nodes.clone()) {
                        tracing::error!("failed to bootstrap: {e:?}");
                    }
                }

                status = Status::Running {
                    table_worker_handle: table_worker.spawn(),
                    pool_worker_handle: pool_worker.spawn(),
                    shutdown,
                };
            },
            Status::Running { .. } => {
                unreachable!("invalid DHT status")
            },
        }

        guard.replace(status);
    }

    async fn shutdown(&self) {
        let mut guard = self.status.lock().await;
        let mut status = guard.take().unwrap();
        match status {
            Status::Running {
                pool_worker_handle,
                table_worker_handle,
                shutdown,
            } => {
                // There may be a race condition if shutdown() is called immediately after
                // start(). Calling notify_waiters() notifies tasks that have called notified().
                // Workers may not have had the chance to do that so we may consider using a
                // notifier for each worker.
                shutdown.notify_waiters();
                let table_request_receiver = table_worker_handle.await.unwrap();
                let pool_task_receiver = pool_worker_handle.await.unwrap();

                status = Status::NotRunning {
                    table_request_receiver,
                    pool_task_receiver,
                };
            },
            Status::NotRunning { .. } => {
                unreachable!("invalid DHT status when shutting down")
            },
        }

        guard.replace(status);
    }
}

#[async_trait]
impl<C> DhtInterface<C> for Dht<C>
where
    C: Collection,
{
    fn init(
        signer: &c![C::SignerInterface],
        sync_query: c![C::ApplicationInterface::SyncExecutor],
        config: Self::Config,
    ) -> Result<Self> {
        let (_, sk) = signer.get_sk();
        let (request_queue_tx, request_queue_rx) = mpsc::channel(1024);
        let table = DhtTable::new(request_queue_tx);

        let (task_queue_tx, task_queue_rx) = mpsc::channel(1024);
        let pool = DhtPool::new(task_queue_tx);

        Ok(Dht {
            address: config.address,
            sk,
            table,
            pool,
            status: Arc::new(Mutex::new(Some(Status::NotRunning {
                table_request_receiver: request_queue_rx,
                pool_task_receiver: task_queue_rx,
            }))),
            sync_query,
            bootstrap_nodes: config.bootstrappers,
        })
    }

    fn put(&self, _: KeyPrefix, key: &[u8], value: &[u8]) {
        let table = self.table.clone();
        match key.try_into() {
            Ok(key) => {
                let value = Bytes::from(value.to_vec());
                tokio::spawn(async move {
                    table.put(key, value).await;
                });
            },
            Err(e) => {
                tracing::error!("`put` failed: {e:?}");
            },
        };
    }

    async fn get(&self, _: KeyPrefix, key: &[u8]) -> Option<TableEntry> {
        let key: TableKey = key.try_into().ok()?;
        let value = self.table.get(key).await?;
        // Todo: table should return TableEntry.
        Some(TableEntry {
            prefix: KeyPrefix::ContentRegistry,
            key: key.to_vec(),
            value: value.to_vec(),
            source: NodePublicKey([9; 32]),
            signature: None,
        })
    }
}

impl<C> ConfigConsumer for Dht<C>
where
    C: Collection,
{
    const KEY: &'static str = "dht";

    type Config = Config;
}
