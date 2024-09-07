use std::io;
use std::net::SocketAddr;

use anyhow::{bail, Error, Result};
use bytes::Bytes;
use fdi::BuildGraph;
use lightning_types::NodeIndex;
pub use lightning_types::RejectReason;
use ready::empty::EmptyReadyState;
use ready::ReadyWaiterState;
use tokio_stream::Stream;

use crate::collection::Collection;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum ServiceScope {
    Broadcast = 0x00,
    BlockstoreServer = 0x01,
    TaskBroker = 0x02,
}

impl TryFrom<u8> for ServiceScope {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::Broadcast),
            0x01 => Ok(Self::BlockstoreServer),
            0x02 => Ok(Self::TaskBroker),
            _ => bail!("invalid scope value: {value:?}"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct RequestHeader {
    pub peer: NodeIndex,
    pub bytes: Bytes,
}

/// Defines the connection pool.
#[interfaces_proc::blank]
pub trait PoolInterface<C: Collection>: BuildGraph + Send + Sync + Sized + 'static {
    type EventHandler: EventHandlerInterface;
    type Requester: RequesterInterface;
    type Responder: ResponderInterface;

    #[blank(EmptyReadyState)]
    type ReadyState: ReadyWaiterState;

    fn open_event(&self, scope: ServiceScope) -> Self::EventHandler;

    fn open_req_res(&self, scope: ServiceScope) -> (Self::Requester, Self::Responder);

    /// Wait for the pool to be ready and return the listen address.
    async fn wait_for_ready(&self) -> io::Result<Self::ReadyState>;

    /// Returns the local address the pool is listening on.
    ///
    /// This is useful for other services to connect to the pool, especially when
    /// the pool is not bound to a specific address (i.e. `0.0.0.0:0` or `[::]:0`),
    /// in which case the OS will assign a random available port.
    fn listen_address(&self) -> Option<SocketAddr>;

    /// Returns the list of connected peers.
    async fn connected_peers(&self) -> Result<Vec<NodeIndex>>;
}

#[interfaces_proc::blank]
pub trait EventHandlerInterface: Send + Sync {
    fn send_to_all<F: Fn(NodeIndex) -> bool + Send + Sync + 'static>(
        &self,
        payload: Bytes,
        filter: F,
    );
    fn send_to_one(&self, node: NodeIndex, payload: Bytes);
    async fn receive(&mut self) -> Option<(NodeIndex, Bytes)>;
}

#[interfaces_proc::blank]
pub trait RequesterInterface: Clone + Send + Sync + 'static {
    type Response: ResponseInterface;
    #[blank(async { Ok(Default::default()) })]
    fn request(
        &self,
        destination: NodeIndex,
        request: Bytes,
    ) -> impl futures::Future<Output = io::Result<Self::Response>> + Send;
}

#[interfaces_proc::blank]
pub trait ResponseInterface: Send + Sync + 'static {
    #[blank(tokio_stream::Empty<io::Result<Bytes>>)]
    type Body: Stream<Item = io::Result<Bytes>> + Send + Unpin;

    fn status_code(&self) -> Result<(), RejectReason>;
    fn body(self) -> Self::Body;
}

#[interfaces_proc::blank]
pub trait ResponderInterface: Send + Sync + 'static {
    type Request: RequestInterface;
    async fn get_next_request(&mut self) -> io::Result<(RequestHeader, Self::Request)>;
}

#[interfaces_proc::blank]
pub trait RequestInterface: Send + Sync + 'static {
    fn reject(self, reason: RejectReason);
    async fn send(&mut self, frame: Bytes) -> io::Result<()>;
}
