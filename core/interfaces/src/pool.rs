use async_trait::async_trait;
use bytes::Bytes;
use lightning_types::NodeIndex;
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

pub enum ServiceScope {
    Broadcast,
    /// Fetcher.
    BlockstoreServer,
}

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

    fn open_event(scope: ServiceScope) -> Self::EventHandler;

    fn open_req_res(scope: ServiceScope) -> (Self::Requester, Self::Responder);
}

#[async_trait]
#[infusion::blank]
pub trait EventHandler {
    fn send_to_all<F: Fn(NodeIndex) -> bool>(&self, payload: Vec<u8>, filter: F);
    fn send_to_one(&self, node: NodeIndex, payload: Vec<u8>);
    async fn receive(&self) -> (NodeIndex, Vec<u8>);
}

#[async_trait]
#[infusion::blank]
pub trait Requester: Clone + Send + Sync {
    type Response: Response;
    async fn request(&self, destination: NodeIndex, request: Vec<u8>) -> Self::Response;
}

#[infusion::blank]
pub trait Response: Send + Sync {
    type Body<S: Stream<Item = Bytes>> = infusion::Blank<S>;
    fn status_code(&self) -> Result<(), RejectReason>;
    fn body<S: Stream<Item = Bytes>>(self) -> Self::Body<S>;
}

#[async_trait]
#[infusion::blank]
pub trait Responder {
    type Request: Request;
    async fn get_next_request(&mut self) -> (Bytes, Self::Request);
}

#[async_trait]
#[infusion::blank]
pub trait Request: Send + Sync {
    fn reject(self, reason: RejectReason);
    async fn send(&self, frame: Vec<u8>);
}
