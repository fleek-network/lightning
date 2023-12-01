pub mod bucket;
pub mod distance;
mod manager;
pub mod worker;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use fleek_crypto::NodePublicKey;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::ApplicationInterface;
pub use manager::{Event, StdManager};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot, Notify};

use crate::pool::{DhtPool, Pool};
use crate::table::manager::Manager;
use crate::table::worker::{Request, TableKey, TableWorker};

pub fn create_table_and_worker<C, M, P>(
    us: TableKey,
    sync_query: c!(C::ApplicationInterface::SyncExecutor),
    pool: P,
    manager: M,
    event_queue: Receiver<Event>,
    shutdown: Arc<Notify>,
) -> (DhtTable, TableWorker<C, M, P>)
where
    C: Collection,
    M: Manager,
    P: Pool,
{
    let (request_queue_tx, request_queue_rx) = mpsc::channel(1024);

    let worker = TableWorker::new(
        us,
        sync_query,
        manager,
        pool,
        request_queue_rx,
        event_queue,
        shutdown,
    );
    (
        DhtTable {
            request_queue: request_queue_tx,
        },
        worker,
    )
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeInfo {
    // Todo: remove address.
    pub address: SocketAddr,
    pub index: NodeIndex,
    pub key: NodePublicKey,
    pub last_responded: Option<u64>,
}

#[async_trait]
pub trait Table: Clone + Send + Sync + 'static {
    async fn get(&self, key: TableKey) -> Option<Bytes>;

    async fn put(&self, key: TableKey, value: Bytes);

    async fn local_get(&self, key: TableKey) -> Option<Bytes>;

    async fn closest_contacts(&self, key: TableKey) -> Result<Vec<NodeIndex>>;

    fn try_local_put(&self, value: Bytes) -> Result<()>;

    fn bootstrap(&self, bootstrap_nodes: Vec<NodeIndex>) -> Result<()>;
}

#[derive(Clone)]
pub struct DhtTable {
    request_queue: Sender<Request>,
}

impl DhtTable {
    pub fn new(request_queue: Sender<Request>) -> Self {
        Self { request_queue }
    }
}

#[async_trait]
impl Table for DhtTable {
    async fn get(&self, key: TableKey) -> Option<Bytes> {
        let (respond_tx, respond_rx) = oneshot::channel();
        if self
            .request_queue
            .send(Request::Get {
                key: rand::random(),
                local: false,
                respond: respond_tx,
            })
            .await
            .is_err()
        {
            return None;
        }
        match respond_rx.await.ok()? {
            Ok(value) => value,
            Err(e) => {
                tracing::debug!("failed to find entry for key {key:?}: {e:?}");
                None
            },
        }
    }

    async fn put(&self, key: TableKey, value: Bytes) {
        let _ = self
            .request_queue
            .send(Request::Put {
                key,
                value,
                local: false,
            })
            .await;
    }

    async fn local_get(&self, key: TableKey) -> Option<Bytes> {
        let (respond_tx, respond_rx) = oneshot::channel();
        if self
            .request_queue
            .send(Request::Get {
                key: rand::random(),
                local: true,
                respond: respond_tx,
            })
            .await
            .is_err()
        {
            return None;
        }
        match respond_rx.await.ok()? {
            Ok(value) => value,
            Err(e) => {
                tracing::debug!("failed to find entry for key {key:?}: {e:?}");
                None
            },
        }
    }

    async fn closest_contacts(&self, key: TableKey) -> Result<Vec<NodeIndex>> {
        let (respond_tx, respond_rx) = oneshot::channel();
        self.request_queue
            .send(Request::ClosestContacts {
                key,
                respond: respond_tx,
            })
            .await?;
        respond_rx.await.map_err(Into::into)
    }

    fn try_local_put(&self, value: Bytes) -> Result<()> {
        self.request_queue
            .try_send(Request::Put {
                key: rand::random(),
                value,
                local: true,
            })
            .map_err(Into::into)
    }

    fn bootstrap(&self, bootstrappers: Vec<NodeIndex>) -> Result<()> {
        self.request_queue
            .try_send(Request::Bootstrap {
                bootstrap_nodes: bootstrappers,
            })
            .map_err(Into::into)
    }
}
