use affair::Socket;
use anyhow::Result;
use fdi::{Cloned, RefMut};
use futures::Future;
use infusion::c;
use lightning_types::{PeerRequestError, ServerRequest};
use tokio::sync::broadcast;

use crate::infu_collection::Collection;
use crate::{
    BlockStoreInterface,
    ConfigConsumer,
    ConfigProviderInterface,
    PoolInterface,
    ReputationAggregatorInterface,
    ShutdownWaiter,
};

pub type BlockStoreServerSocket =
    Socket<ServerRequest, broadcast::Receiver<Result<(), PeerRequestError>>>;

#[infusion::service]
pub trait BlockStoreServerInterface<C: Collection>: Sized + Send + Sync + ConfigConsumer {
    fn _init(
        config: ::ConfigProviderInterface,
        blockstre: ::BlockStoreInterface,
        pool: ::PoolInterface,
        rep_aggregator: ::ReputationAggregatorInterface,
    ) {
        Self::init(
            config.get::<Self>(),
            blockstre.clone(),
            pool,
            rep_aggregator.get_reporter(),
        )
    }

    fn init(
        config: Self::Config,
        blockstore: C::BlockStoreInterface,
        pool: &C::PoolInterface,
        rep_reporter: c![C::ReputationAggregatorInterface::ReputationReporter],
    ) -> anyhow::Result<Self>;

    #[blank = async { }]
    #[allow(clippy::manual_async_fn)]
    fn start(this: RefMut<Self>, waiter: Cloned<ShutdownWaiter>)
    -> impl Future<Output = ()> + Send;

    fn get_socket(&self) -> BlockStoreServerSocket;
}
