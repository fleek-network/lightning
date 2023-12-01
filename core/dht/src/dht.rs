use std::future::Future;
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::pin::Pin;
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
    ReputationAggregatorInterface,
    SignerInterface,
    SyncQueryRunnerInterface,
    WithStartAndShutdown,
};
use tokio::net::UdpSocket;
use tokio::sync::mpsc::Receiver;
use tokio::sync::{mpsc, Mutex, Notify};

use crate::config::Config;
use crate::network::UdpTransport;
use crate::pool::{DhtPool, Looker, PoolWorker};
use crate::table::worker::{Request, TableKey, TableWorker};
use crate::table::{DhtTable, NodeInfo, StdManager, Table};
use crate::{pool, table};

/// Maintains the DHT.
#[allow(clippy::type_complexity, unused)]
pub struct Dht<C: Collection> {
    table: DhtTable,
    table_request_receiver: Arc<Mutex<Option<Receiver<Request>>>>,
    table_worker: Arc<Mutex<Option<TableWorker<C, StdManager, DhtPool>>>>,
    pool_worker: Arc<
        Mutex<
            Option<
                PoolWorker<
                    Looker<C, DhtTable, UdpTransport<c![C::ApplicationInterface::SyncExecutor]>>,
                    DhtTable,
                    UdpTransport<c![C::ApplicationInterface::SyncExecutor]>,
                >,
            >,
        >,
    >,
    network_secret_key: NodeSecretKey,
    nodes: Vec<NodePublicKey>,
    sync_query: c![C::ApplicationInterface::SyncExecutor],
    is_running: Arc<Mutex<bool>>,
    shutdown_notify: Arc<Notify>,
    collection: PhantomData<C>,
}

#[async_trait]
impl<C> WithStartAndShutdown for Dht<C>
where
    C: Collection,
{
    fn is_running(&self) -> bool {
        todo!()
    }

    async fn start(&self) {
        // Todo: Fix this because index may not be available.
        let local_pk = self.network_secret_key.to_pk();
        let local_index = self.sync_query.pubkey_to_index(local_pk).unwrap();
        {
            let mut pool_worker_guard = self.pool_worker.lock().await;
            if pool_worker_guard.is_none() {
                // Todo: clean up socket in shutdown.
                let address: SocketAddr = "0.0.0.0:0".parse().unwrap();
                let udp_socket = Arc::new(UdpSocket::bind(address).await.unwrap());
                let socket = UdpTransport::new(udp_socket, self.sync_query.clone());

                let looker = Looker::new(
                    local_index,
                    self.table.clone(),
                    socket.clone(),
                    self.sync_query.clone(),
                );

                let (event_queue_tx, event_queue_rx) = mpsc::channel(1024);
                let (pool, pool_worker) = pool::create_pool_and_worker(
                    local_index,
                    looker,
                    self.table.clone(),
                    socket.clone(),
                    event_queue_tx,
                    self.shutdown_notify.clone(),
                );

                pool_worker_guard.replace(pool_worker);

                let request_queue = self.table_request_receiver.lock().await.take().unwrap();
                let table_worker = TableWorker::new(
                    local_pk.0,
                    self.sync_query.clone(),
                    StdManager::new(local_pk),
                    pool,
                    request_queue,
                    event_queue_rx,
                    self.shutdown_notify.clone(),
                );

                self.table_worker.lock().await.replace(table_worker);
            }
        }
    }

    async fn shutdown(&self) {
        todo!()
    }
}

#[async_trait]
impl<C> DhtInterface<C> for Dht<C>
where
    C: Collection,
{
    fn init(
        signer: &c![C::SignerInterface],
        _: c![C::TopologyInterface],
        _: c!(C::ReputationAggregatorInterface::ReputationReporter),
        _: c![C::ReputationAggregatorInterface::ReputationQuery],
        sync_query: c![C::ApplicationInterface::SyncExecutor],
        config: Self::Config,
    ) -> Result<Self> {
        let (_, sk) = signer.get_sk();
        let (request_queue_tx, request_queue_rx) = mpsc::channel(1024);
        let table = DhtTable::new(request_queue_tx);
        Ok(Dht {
            table,
            table_request_receiver: Arc::new(Mutex::new(Some(request_queue_rx))),
            table_worker: Arc::new(Mutex::new(None)),
            pool_worker: Arc::new(Mutex::new(None)),
            network_secret_key: sk,
            sync_query,
            nodes: config.bootstrappers,
            is_running: Arc::new(Mutex::new(false)),
            shutdown_notify: Arc::new(Notify::new()),
            collection: PhantomData,
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
