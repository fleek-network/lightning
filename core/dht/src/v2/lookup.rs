use std::net::IpAddr;
use std::sync::Arc;

use bytes::Bytes;
use tokio::net::UdpSocket;
use tokio::sync::oneshot;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::SyncQueryRunnerInterface;
use lightning_interfaces::types::NodeIndex;

pub type LookUpValueRespond =  oneshot::Sender<Result<Option<Bytes>>>;
pub type LookUpProviderRespond =  oneshot::Sender<Result<Vec<NodeIndex>>>;

pub type Result<T> = std::result::Result<T, Error>;

/// Look-up response.
pub struct Response {
    id: u64,
    payload: Bytes
}

/// Look-up response payload.
struct Payload {
    value: Bytes,
    peers: Vec<NodeIndex>,
}

pub enum Error {
    Canceled,
}

pub trait LookupInterface<C: Collection>: Clone {
    fn init(
        table_client: (),
        socket: Arc<UdpSocket>,
        sync_query: c!(C::ApplicationInterface::SyncExecutor),
    ) -> Self;
    async fn find_node(&self, hash: u32, respond: LookUpProviderRespond) -> Result<()>;
    async fn find_value(&self, hash: u32, respond: LookUpValueRespond) -> Result<()>;
}
