use async_trait::async_trait;
use infusion::c;

use crate::infu_collection::Collection;
use crate::{
    ApplicationInterface,
    ConfigConsumer,
    ConfigProviderInterface,
    NotifierInterface,
    ReputationAggregatorInterface,
    SignerInterface,
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
        notifier: ::NotifierInterface,
        signer: ::SignerInterface,
    ) {
        Self::init(
            config.get::<Self>(),
            app.sync_query(),
            rep_aggregator.get_reporter(),
            notifier.clone(),
            signer,
        )
    }

    fn init(
        config: Self::Config,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
        notifier: C::NotifierInterface,
        signer: &C::SignerInterface,
    ) -> anyhow::Result<Self>;
}
