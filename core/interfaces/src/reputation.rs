use std::time::Duration;

use async_trait::async_trait;
use fleek_crypto::NodePublicKey;

use crate::{
    application::SyncQueryRunnerInterface, config::ConfigConsumer, notifier::NotifierInterface,
    signer::SubmitTxSocket,
};

#[async_trait]
pub trait ReputationAggregatorInterface: ConfigConsumer + Sized {
    // -- DYNAMIC TYPES

    /// The notifier can be used to receive notifications on and before epoch changes.
    type Notifier: NotifierInterface;

    // -- BOUNDED TYPES

    /// The reputation reporter can be used by our system to report the reputation of other
    type ReputationReporter: ReputationReporterInterface;

    /// The query runner can be used to query the local reputation of other nodes.
    type ReputationQuery: ReputationQueryInteface;

    /// Create a new reputation
    async fn init(
        config: Self::Config,
        submit_tx: SubmitTxSocket,
        notifier: Self::Notifier,
    ) -> anyhow::Result<Self>;

    /// Called by the scheduler to notify that it is time to submit the aggregation, to do
    /// so one should use the [`SubmitTxSocket`] that is passed during the initialization
    /// to submit a transaction to the consensus.
    fn submit_aggregation(&self);

    /// Returns a reputation reporter that can be used to capture interactions that we have
    /// with another peer.
    fn get_reporter(&self) -> Self::ReputationReporter;

    /// Returns a reputation query that can be used to answer queries about the local
    /// reputation we have of another peer.
    fn get_query(&self) -> Self::ReputationQuery;
}

/// Used to answer queries about the (local) reputation of other nodes, this queries should
/// be as real-time as possible, meaning that the most recent data captured by the reporter
/// should be taken into account at this layer.
pub trait ReputationQueryInteface: Clone {
    /// The application layer's synchronize query runner.
    type SyncQuery: SyncQueryRunnerInterface;

    /// Returns the reputation of the provided node locally.
    fn get_reputation_of(&self, peer: &NodePublicKey) -> Option<u8>;
}

/// Reputation reporter is a cheaply cleanable object which can be used to report the interactions
/// that we have with another peer, this interface allows a reputation aggregator to spawn many
/// reporters which can use any method to report the data they capture to their aggregator so
/// that it can send it to the application layer.
pub trait ReputationReporterInterface: Clone {
    /// Report a satisfactory (happy) interaction with the given peer. Used for up time.
    fn report_sat(&self, peer: &NodePublicKey, weight: Weight);

    /// Report a unsatisfactory (happy) interaction with the given peer. Used for down time.
    fn report_unsat(&self, peer: &NodePublicKey, weight: Weight);

    /// Report a latency which we witnessed from another peer.
    fn report_latency(&self, peer: &NodePublicKey, latency: Duration);

    /// Report the number of (healthy) bytes which we received from another peer.
    fn report_bytes_received(&self, peer: &NodePublicKey, bytes: u64, duration: Option<Duration>);

    /// Report the number of (healthy) bytes which we sent from another peer.
    fn report_bytes_sent(&self, peer: &NodePublicKey, bytes: u64, duration: Option<Duration>);

    /// Report the number of hops we have witnessed to the given peer.
    fn report_hops(&self, peer: &NodePublicKey, hops: u8);
}

// TODO: Move to types/reputation.rs as `ReputationWeight`.
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq)]
pub enum Weight {
    Weak,
    Strong,
    VeryStrong,
    Provable,
}
