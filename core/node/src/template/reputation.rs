use std::time::Duration;

use async_trait::async_trait;
use draco_interfaces::{
    config::ConfigConsumer, reputation::ReputationAggregatorInterface, signer::SubmitTxSocket,
    ReputationQueryInteface, ReputationReporterInterface, Weight,
};
use fleek_crypto::NodePublicKey;

use super::{application::QueryRunner, config::Config};

#[derive(Clone)]
pub struct ReputationAggregator {}

#[async_trait]
impl ReputationAggregatorInterface for ReputationAggregator {
    /// The reputation reporter can be used by our system to report the reputation of other
    type ReputationReporter = MyReputationReporter;

    /// The query runner can be used to query the local reputation of other nodes.
    type ReputationQuery = MyReputationQuery;

    /// Create a new reputation
    async fn init(_config: Self::Config, _submit_tx: SubmitTxSocket) -> anyhow::Result<Self> {
        todo!()
    }

    fn submit_aggregation(&self) {
        todo!()
    }

    /// Returns a reputation reporter that can be used to capture interactions that we have
    /// with another peer.
    fn get_reporter(&self) -> Self::ReputationReporter {
        todo!()
    }
}

impl ConfigConsumer for ReputationAggregator {
    const KEY: &'static str = "reputation";

    type Config = Config;
}

#[derive(Clone)]
pub struct MyReputationQuery {}

impl ReputationQueryInteface for MyReputationQuery {
    /// The application layer's synchronize query runner.
    type SyncQuery = QueryRunner;

    /// Returns the reputation of the provided node locally.
    fn get_reputation_of(&self, _peer: &NodePublicKey) -> Option<u128> {
        todo!()
    }
}

#[derive(Clone)]
pub struct MyReputationReporter {}

impl ReputationReporterInterface for MyReputationReporter {
    /// Report a satisfactory (happy) interaction with the given peer.
    fn report_sat(&self, _peer: &NodePublicKey, _weight: Weight) {
        todo!()
    }

    /// Report a unsatisfactory (happy) interaction with the given peer.
    fn report_unsat(&self, _peer: &NodePublicKey, _weight: Weight) {
        todo!()
    }

    /// Report a latency which we witnessed from another peer.
    fn report_latency(&self, _peer: &NodePublicKey, _latency: Duration) {
        todo!()
    }

    /// Report the number of (healthy) bytes which we received from another peer.
    fn report_bytes_received(
        &self,
        _peer: &NodePublicKey,
        _bytes: u64,
        _duration: Option<Duration>,
    ) {
        todo!()
    }

    fn report_bytes_sent(&self, _peer: &NodePublicKey, _bytes: u64, _duration: Option<Duration>) {
        todo!()
    }

    fn report_hops(&self, _peer: &NodePublicKey, _hops: u8) {
        todo!()
    }
}
