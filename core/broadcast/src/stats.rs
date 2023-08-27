use std::sync::Arc;

use dashmap::DashMap;
use derive_more::AddAssign;
use fxhash::FxBuildHasher;
use lightning_interfaces::types::NodeIndex;

#[derive(Default, Clone)]
pub struct Stats {
    inner: Arc<DashMap<NodeIndex, ConnectionStats, FxBuildHasher>>,
}

/// A bunch of statistics that we gather from a peer throughout the life of the gossip.
#[derive(Default, AddAssign, Copy, Clone, Debug)]
pub struct ConnectionStats {
    /// How many things have we advertised to this node.
    pub advertisements_received_from_us: usize,
    /// How many things has this peer advertised to us.
    pub advertisements_received_from_peer: usize,
    /// How many `WANT`s have we sent to this node.
    pub wants_received_from_us: usize,
    /// How many `WANT`s has this peer sent our way.
    pub wants_received_from_peer: usize,
    /// Valid messages sent by this node to us.
    pub messages_received_from_peer: usize,
    /// Number of messages we have received from this peer that
    /// we did not continue propagating.
    pub invalid_messages_received_from_peer: usize,
}

impl Stats {
    /// Report some new stats.
    pub fn report(&self, peer: NodeIndex, stats: ConnectionStats) {
        *self.inner.entry(peer).or_default() += stats;
    }
}
