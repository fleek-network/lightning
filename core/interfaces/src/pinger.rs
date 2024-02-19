use infusion::c;

use crate::infu_collection::Collection;
use crate::{
    ApplicationInterface,
    ConfigConsumer,
    ConfigProviderInterface,
    KeystoreInterface,
    NotifierInterface,
    ReputationAggregatorInterface,
    WithStartAndShutdown,
};

#[infusion::service]
pub trait PingerInterface<C: Collection>:
    WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync
{
    fn _init(
        config: ::ConfigProviderInterface,
        app: ::ApplicationInterface,
        rep_aggregator: ::ReputationAggregatorInterface,
        notifier: ::NotifierInterface,
        keystore: ::KeystoreInterface,
    ) {
        Self::init(
            config.get::<Self>(),
            app.sync_query(),
            rep_aggregator.get_reporter(),
            notifier.clone(),
            keystore.clone(),
        )
    }

    fn init(
        config: Self::Config,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
        notifier: C::NotifierInterface,
        keystore: C::KeystoreInterface,
    ) -> anyhow::Result<Self>;
}
