use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use fleek_crypto::NodePublicKey;
use lightning_interfaces::types::NodeIndex;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct State {
    pub connections: HashMap<NodeIndex, ConnectionInfo>,
    pub event_queue_cap: usize,
    pub event_queue_max_cap: usize,
    pub endpoint_task_queue_cap: usize,
    pub endpoint_task_queue_max_cap: usize,
    pub ongoing_endpoint_async_tasks: usize,
}

pub struct EventReceiverInfo {
    pub connections: HashMap<NodeIndex, ConnectionInfo>,
    pub endpoint_queue_cap: usize,
    pub endpoint_queue_max_cap: usize,
    pub ongoing_endpoint_async_tasks: usize,
}

#[derive(Default, Deserialize, Serialize)]
pub struct ConnectionInfo {
    pub from_topology: bool,
    pub pinned: bool,
    pub peer: Option<NodeInfo>,
    pub actual_connections: Vec<TransportConnectionInfo>,
}

pub struct EndpointInfo {
    pub ongoing_async_tasks: usize,
    pub connections: HashMap<NodeIndex, Vec<TransportConnectionInfo>>,
}

#[derive(Deserialize, Serialize)]
pub struct TransportConnectionInfo {
    pub redundant: bool,
    pub request_queue_cap: usize,
    pub request_queue_max_cap: usize,
    pub stats: Stats,
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

#[derive(Debug)]
pub struct DialInfo {
    pub num_tries: u32,
    pub last_try: Instant,
}
