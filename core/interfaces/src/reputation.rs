use std::collections::BTreeMap;
use std::time::Duration;

use fdi::BuildGraph;
use lightning_types::{NodeIndex, ReputationMeasurements};

use crate::components::NodeComponents;

#[interfaces_proc::blank]
pub trait ReputationAggregatorInterface<C: NodeComponents>: BuildGraph {
    /// The reputation reporter can be used by our system to report the reputation of other
    type ReputationReporter: ReputationReporterInterface;

    /// The query runner can be used to query the local reputation of other nodes.
    type ReputationQuery: ReputationQueryInteface;

    /// Returns a reputation reporter that can be used to capture interactions that we have
    /// with another peer.
    #[blank = Default::default()]
    fn get_reporter(&self) -> Self::ReputationReporter;

    /// Returns a reputation query that can be used to answer queries about the local
    /// reputation we have of another peer.
    #[blank = Default::default()]
    fn get_query(&self) -> Self::ReputationQuery;
}

/// Used to answer queries about the (local) reputation of other nodes, this queries should
/// be as real-time as possible, meaning that the most recent data captured by the reporter
/// should be taken into account at this layer.
#[interfaces_proc::blank]
pub trait ReputationQueryInteface: Clone + Send + Sync {
    /// Returns the reputation of the provided node locally.
    fn get_reputation_of(&self, peer: &NodeIndex) -> Option<u8>;

    /// Returns reputation measurements for all peers.
    fn get_measurements(&self) -> BTreeMap<NodeIndex, ReputationMeasurements>;
}

/// Reputation reporter is a cheaply cleanable object which can be used to report the interactions
/// that we have with another peer, this interface allows a reputation aggregator to spawn many
/// reporters which can use any method to report the data they capture to their aggregator so
/// that it can send it to the application layer.
#[interfaces_proc::blank]
pub trait ReputationReporterInterface: Clone + Send + Sync {
    /// Report a satisfactory (happy) interaction with the given peer. Used for up time.
    fn report_sat(&self, peer: NodeIndex, weight: Weight);

    /// Report a unsatisfactory (happy) interaction with the given peer. Used for down time.
    fn report_unsat(&self, peer: NodeIndex, weight: Weight);

    /// Report a ping interaction with another peer and the latency if the peer responded.
    /// `None` indicates that the peer did not respond.
    fn report_ping(&self, peer: NodeIndex, latency: Option<Duration>);

    /// Report the number of (healthy) bytes which we received from another peer.
    fn report_bytes_received(&self, peer: NodeIndex, bytes: u64, duration: Option<Duration>);

    /// Report the number of (healthy) bytes which we sent from another peer.
    fn report_bytes_sent(&self, peer: NodeIndex, bytes: u64, duration: Option<Duration>);

    /// Report the number of hops we have witnessed to the given peer.
    fn report_hops(&self, peer: NodeIndex, hops: u8);
}

// TODO: Move to types/reputation.rs as `ReputationWeight`.
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq)]
pub enum Weight {
    Weak,
    Strong,
    VeryStrong,
    Provable,
}
