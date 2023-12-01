use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use affair::Task;
use anyhow::Result;
use async_trait::async_trait;
use fleek_crypto::NodeSecretKey;
use lightning_interfaces::dht::{DhtInterface, DhtSocket};
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::types::{DhtRequest, DhtResponse, KeyPrefix, TableEntry};
use lightning_interfaces::{ConfigConsumer, ReputationAggregatorInterface, WithStartAndShutdown};
use tokio::sync::{mpsc, Notify};

use crate::config::Config;
use crate::table::NodeInfo;

/// Maintains the DHT.
#[allow(clippy::type_complexity, unused)]
pub struct Dht<C: Collection> {
    socket: DhtSocket,
    socket_rx: Arc<Mutex<Option<mpsc::Receiver<Task<DhtRequest, DhtResponse>>>>>,
    network_secret_key: NodeSecretKey,
    nodes: Arc<Mutex<Option<Vec<NodeInfo>>>>,
    is_running: Arc<Mutex<bool>>,
    shutdown_notify: Arc<Notify>,
    collection: PhantomData<C>,
}

impl<C: Collection> Dht<C> {
    /// Return one value associated with the given key.
    pub async fn get(&self, prefix: KeyPrefix, key: &[u8]) -> Option<TableEntry> {
        match self
            .socket
            .run(DhtRequest::Get {
                prefix,
                key: key.to_vec(),
            })
            .await
        {
            Ok(DhtResponse::Get(value)) => value,
            Err(e) => {
                tracing::error!("failed to get entry for key {key:?}: {e:?}");
                None
            },
            Ok(_) => unreachable!(),
        }
    }

    /// Put a key-value pair into the DHT.
    pub fn put(&self, prefix: KeyPrefix, key: &[u8], value: &[u8]) {
        let socket = self.socket.clone();
        let key = key.to_vec();
        let value = value.to_vec();
        tokio::spawn(async move {
            if let Err(e) = socket.enqueue(DhtRequest::Put { prefix, key, value }).await {
                tracing::error!("failed to put entry: {e:?}");
            }
        });
    }

    /// Start bootstrap task.
    /// If bootstrapping is in process, this request will be ignored.
    pub async fn bootstrap(&self) -> Result<()> {
        todo!()
    }
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for Dht<C> {
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

#[async_trait]
impl<C: Collection> DhtInterface<C> for Dht<C> {
    fn init(
        _: &c![C::SignerInterface],
        _: c![C::TopologyInterface],
        _: c!(C::ReputationAggregatorInterface::ReputationReporter),
        _: c![C::ReputationAggregatorInterface::ReputationQuery],
        _: Self::Config,
    ) -> Result<Self> {
        todo!()
    }

    fn get_socket(&self) -> DhtSocket {
        self.socket.clone()
    }
}

impl<C: Collection> ConfigConsumer for Dht<C> {
    const KEY: &'static str = "dht";

    type Config = Config;
}
