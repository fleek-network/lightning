use std::io;

use anyhow::{bail, Error};
use async_trait::async_trait;
use bytes::Bytes;
use lightning_types::NodeIndex;
use serde::{Deserialize, Serialize};
use tokio_stream::Stream;

use crate::infu_collection::{c, Collection};
use crate::signer::SignerInterface;
use crate::{
    ApplicationInterface,
    ConfigConsumer,
    ConfigProviderInterface,
    NotifierInterface,
    TopologyInterface,
    WithStartAndShutdown,
};

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

#[derive(Clone, Copy, Deserialize, Serialize, Debug)]
#[repr(u8)]
#[non_exhaustive]
pub enum RejectReason {
    Other,
}

/// Defines the connection pool.
#[infusion::service]
pub trait PoolInterface<C: Collection>:
    WithStartAndShutdown + ConfigConsumer + Send + Sync + Sized
{
    fn _init(
        config: ::ConfigProviderInterface,
        signer: ::SignerInterface,
        app: ::ApplicationInterface,
        notifier: ::NotifierInterface,
        topology: ::TopologyInterface,
    ) {
        Self::init(
            config.get::<Self>(),
            signer,
            app.sync_query(),
            notifier.clone(),
            topology.clone(),
        )
    }

    type EventHandler: EventHandler;
    type Requester: Requester;
    type Responder: Responder;

    fn init(
        config: Self::Config,
        signer: &c!(C::SignerInterface),
        sqr: c!(C::ApplicationInterface::SyncExecutor),
        notifier: c!(C::NotifierInterface),
        topology: c!(C::TopologyInterface),
    ) -> anyhow::Result<Self>;

    fn open_event(&self, scope: ServiceScope) -> Self::EventHandler;

    fn open_req_res(&self, scope: ServiceScope) -> (Self::Requester, Self::Responder);
}

#[async_trait]
#[infusion::blank]
pub trait EventHandler: Send + Sync {
    fn send_to_all<F: Fn(NodeIndex) -> bool + Send + Sync + 'static>(
        &self,
        payload: Bytes,
        filter: F,
    );
    fn send_to_one(&self, node: NodeIndex, payload: Bytes);
    async fn receive(&mut self) -> Option<(NodeIndex, Bytes)>;
}

#[async_trait]
#[infusion::blank]
pub trait Requester: Clone + Send + Sync {
    type Response: Response;
    async fn request(&self, destination: NodeIndex, request: Bytes) -> io::Result<Self::Response>;
}

#[infusion::blank]
pub trait Response: Send + Sync {
    type Body: Stream<Item = io::Result<Bytes>> + Send + Unpin =
        tokio_stream::Empty<io::Result<Bytes>>;
    fn status_code(&self) -> Result<(), RejectReason>;
    fn body(self) -> Self::Body;
}

#[async_trait]
#[infusion::blank]
pub trait Responder {
    type Request: Request;
    async fn get_next_request(&mut self) -> io::Result<(Bytes, Self::Request)>;
}

#[async_trait]
#[infusion::blank]
pub trait Request: Send + Sync {
    fn reject(self, reason: RejectReason);
    async fn send(&mut self, frame: Bytes) -> io::Result<()>;
}
