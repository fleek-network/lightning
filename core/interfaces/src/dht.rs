use affair::Socket;
use infusion::c;

use crate::infu_collection::Collection;
use crate::types::{DhtRequest, DhtResponse};
use crate::{
    ConfigConsumer,
    ConfigProviderInterface,
    ReputationAggregatorInterface,
    SignerInterface,
    TopologyInterface,
    WithStartAndShutdown,
};

pub type DhtSocket = Socket<DhtRequest, DhtResponse>;

#[infusion::service]
pub trait DhtInterface<C: Collection>:
    ConfigConsumer + WithStartAndShutdown + Sized + Send + Sync
{
    fn _init(
        signer: ::SignerInterface,
        topology: ::TopologyInterface,
        config: ::ConfigProviderInterface,
        rep_aggregator: ::ReputationAggregatorInterface,
    ) {
        Self::init(
            signer,
            topology.clone(),
            rep_aggregator.get_reporter(),
            rep_aggregator.get_query(),
            config.get::<Self>(),
        )
    }

    fn init(
        signer: &c!(C::SignerInterface),
        topology: c!(C::TopologyInterface),
        rep_reporter: c![C::ReputationAggregatorInterface::ReputationReporter],
        local_rep_query: c![C::ReputationAggregatorInterface::ReputationQuery],
        config: Self::Config,
    ) -> anyhow::Result<Self>;

    fn get_socket(&self) -> DhtSocket;
}
