use affair::Socket;
use anyhow::Result;
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
    WithStartAndShutdown,
};

pub type BlockStoreServerSocket =
    Socket<ServerRequest, broadcast::Receiver<Result<(), PeerRequestError>>>;

#[infusion::service]
pub trait BlockStoreServerInterface<C: Collection>:
    Sized + Send + Sync + ConfigConsumer + WithStartAndShutdown
{
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

    fn get_socket(&self) -> BlockStoreServerSocket;
}
