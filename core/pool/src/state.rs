use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use fleek_crypto::NodePublicKey;
use lightning_interfaces::types::NodeIndex;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

use crate::overlay::ConnectionInfo;

pub type QuerySender = Sender<Query>;

pub enum Query {
    /// Request for the entire state of the pool.
    State { respond: oneshot::Sender<State> },
    /// Request for information and stats from a specific peer.
    Peer {
        index: NodeIndex,
        respond: oneshot::Sender<Option<PeerInfo>>,
    },
}

#[derive(Deserialize, Serialize)]
pub struct State {
    pub logical_connections: HashMap<NodeIndex, ConnectionInfo>,
    pub actual_connections: HashMap<NodeIndex, PeerInfo>,
    pub redundant_connections: HashMap<NodeIndex, PeerInfo>,
}

#[derive(Default, Deserialize, Serialize)]
pub struct PeerInfo {
    pub info: Option<NodeInfo>,
    pub stats: Option<Stats>,
}

#[derive(Deserialize, Serialize)]
pub struct Stats {
    pub rtt: Duration,
    pub lost_packets: u64,
    pub sent_packets: u64,
    pub congestion_events: u64,
    pub cwnd: u64,
    pub black_holes_detected: u64,
}

/// Info of a node.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeInfo {
    pub index: NodeIndex,
    pub pk: NodePublicKey,
    pub socket_address: SocketAddr,
}
