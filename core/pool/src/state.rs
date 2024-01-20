use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use fleek_crypto::NodePublicKey;
use lightning_interfaces::types::NodeIndex;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

#[allow(dead_code)]
pub type QuerySender = Sender<Query>;

#[allow(dead_code)]
pub enum Query {
    /// Request for the entire state of the pool.
    State { respond: oneshot::Sender<State> },
    /// Request for information and stats from a specific peer.
    Peer {
        index: NodeIndex,
        respond: oneshot::Sender<Option<ConnectionInfo>>,
    },
}

#[derive(Deserialize, Serialize)]
pub struct State {
    pub connections: HashMap<NodeIndex, ConnectionInfo>,
    pub broadcast_queue_cap: usize,
    pub broadcast_queue_max_cap: usize,
    pub send_req_queue_cap: usize,
    pub send_req_queue_max_cap: usize,
}

#[derive(Default, Deserialize, Serialize)]
pub struct ConnectionInfo {
    pub from_topology: bool,
    pub pinned: bool,
    pub peer: Option<NodeInfo>,
    pub work_queue_cap: usize,
    pub work_queue_max_cap: usize,
    pub actual_connections: Vec<TransportConnectionInfo>,
}

#[derive(Deserialize, Serialize)]
pub struct TransportConnectionInfo {
    pub redundant: bool,
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
