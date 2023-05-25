use std::time::Duration;

use async_trait::async_trait;

use crate::{
    application::SyncQueryRunnerInterface, config::ConfigConsumer, identity::PeerId,
    signer::SubmitTxSocket,
};

#[async_trait]
pub trait ReputationAggregatorInterface: ConfigConsumer + Sized {
    /// The reputation reporter can be used by our system to report the reputation of other
    type ReputationReporter: ReputationReporterInterface;

    /// The query runner can be used to query the local reputation of other nodes.
    type ReputationQuery: ReputationQueryInteface;

    /// Create a new reputation
    async fn init(config: Self::Config, submit_tx: SubmitTxSocket) -> anyhow::Result<Self>;

    /// Returns a reputation reporter that can be used to capture interactions that we have
    /// with another peer.
    fn get_reporter(&self) -> Self::ReputationReporter;
}

/// Used to answer queries about the (local) reputation of other nodes, this queries should
/// be as real-time as possible, meaning that the most recent data captured by the reporter
/// should be taken into account at this layer.
pub trait ReputationQueryInteface: Clone {
    /// The application layer's synchronize query runner.
    type SyncQuery: SyncQueryRunnerInterface;

    /// Returns the reputation of the provided node locally.
    fn get_reputation_of(&self, peer: &PeerId) -> u128;
}

/// Reputation reporter is a cheaply cleanable object which can be used to report the interactions
/// that we have with another peer, this interface allows a reputation aggregator to spawn many
/// reporters which can use any method to report the data they capture to their aggregator so
/// that it can send it to the application layer.
pub trait ReputationReporterInterface: Clone {
    /// Report a satisfactory (happy) interaction with the given peer.
    fn report_sat(&self, peer: &PeerId, weight: Weight);

    /// Report a unsatisfactory (happy) interaction with the given peer.
    fn report_unsat(&self, peer: &PeerId, weight: Weight);

    /// Report a latency which we witnessed from another peer.
    fn report_latency(&self, peer: &PeerId, latency: Duration);

    /// Report the number of (healthy) bytes which we received from another peer.
    fn report_bytes_received(&self, peer: &PeerId, bytes: u64);
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq)]
pub enum Weight {
    Weak,
    Strong,
    VeryStrong,
    Provable,
}
