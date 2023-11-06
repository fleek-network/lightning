use async_trait::async_trait;
use infusion::c;

use crate::infu_collection::Collection;
use crate::{
    ApplicationInterface,
    ConfigConsumer,
    ConfigProviderInterface,
    ReputationAggregatorInterface,
    WithStartAndShutdown,
};

#[async_trait]
#[infusion::service]
pub trait PingerInterface<C: Collection>:
    WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync
{
    fn _init(
        config: ::ConfigProviderInterface,
        app: ::ApplicationInterface,
        rep_aggregator: ::ReputationAggregatorInterface,
    ) {
        Self::init(
            config.get::<Self>(),
            app.sync_query(),
            rep_aggregator.get_reporter(),
        )
    }

    fn init(
        config: Self::Config,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
    ) -> anyhow::Result<Self>;
}
