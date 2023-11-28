use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::ApplicationInterface;
use tokio::net::UdpSocket;
use tokio::sync::oneshot;

pub type ValueRespond = oneshot::Sender<Result<Option<Bytes>>>;
pub type ProviderRespond = oneshot::Sender<Result<Vec<NodeIndex>>>;

pub type Result<T> = std::result::Result<T, Error>;

/// Look-up response.
pub struct Response {
    id: u64,
    payload: Bytes,
}

/// Look-up response payload.
struct Payload {
    value: Bytes,
    peers: Vec<NodeIndex>,
}

pub enum Error {
    Canceled,
}

#[async_trait]
pub trait LookupInterface: Clone + Send + Sync + Unpin + 'static {
    async fn find_node(&self, hash: u32) -> Result<Vec<NodeIndex>>;
    async fn find_value(&self, hash: u32) -> Result<Option<Bytes>>;
}

pub struct Looker<C: Collection> {
    sync_query: c![C::ApplicationInterface::SyncExecutor],
    socket: Arc<UdpSocket>,
    _marker: PhantomData<C>,
}

impl<C: Collection> Clone for Looker<C> {
    fn clone(&self) -> Self {
        Self {
            sync_query: self.sync_query.clone(),
            socket: self.socket.clone(),
            _marker: PhantomData,
        }
    }
}

impl<C: Collection> Looker<C> {
    fn new(
        _table_client: (),
        socket: Arc<UdpSocket>,
        sync_query: c!(C::ApplicationInterface::SyncExecutor),
    ) -> Self {
        Self {
            socket,
            sync_query,
            _marker: PhantomData,
        }
    }
}
