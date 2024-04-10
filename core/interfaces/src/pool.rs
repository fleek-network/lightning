use std::io;

use anyhow::{bail, Error};
use bytes::Bytes;
use fdi::BuildGraph;
use lightning_types::NodeIndex;
pub use lightning_types::RejectReason;
use tokio_stream::Stream;

use crate::collection::Collection;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum ServiceScope {
    Broadcast = 0x00,
    /// Fetcher.
    BlockstoreServer = 0x01,
}

impl TryFrom<u8> for ServiceScope {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::Broadcast),
            0x01 => Ok(Self::BlockstoreServer),
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
pub trait PoolInterface<C: Collection>: BuildGraph + Send + Sync + Sized {
    #[blank(crate::_hacks::Blanket)]
    type EventHandler: EventHandlerInterface;
    #[blank(crate::_hacks::Blanket)]
    type Requester: RequesterInterface;
    #[blank(crate::_hacks::Blanket)]
    type Responder: ResponderInterface;

    fn open_event(&self, scope: ServiceScope) -> Self::EventHandler;

    fn open_req_res(&self, scope: ServiceScope) -> (Self::Requester, Self::Responder);
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
pub trait RequesterInterface: Clone + Send + Sync {
    #[blank(crate::_hacks::Blanket)]
    type Response: ResponseInterface;
    async fn request(&self, destination: NodeIndex, request: Bytes) -> io::Result<Self::Response>;
}

#[interfaces_proc::blank]
pub trait ResponseInterface: Send + Sync {
    #[blank(tokio_stream::Empty<io::Result<Bytes>>)]
    type Body: Stream<Item = io::Result<Bytes>> + Send + Unpin;

    fn status_code(&self) -> Result<(), RejectReason>;
    fn body(self) -> Self::Body;
}

#[interfaces_proc::blank]
pub trait ResponderInterface: Send + Sync {
    #[blank(crate::_hacks::Blanket)]
    type Request: RequestInterface;
    async fn get_next_request(&mut self) -> io::Result<(RequestHeader, Self::Request)>;
}

#[interfaces_proc::blank]
pub trait RequestInterface: Send + Sync {
    fn reject(self, reason: RejectReason);
    async fn send(&mut self, frame: Bytes) -> io::Result<()>;
}
