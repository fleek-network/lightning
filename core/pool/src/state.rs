use std::collections::HashMap;
use std::time::Duration;

use lightning_interfaces::types::NodeIndex;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

use crate::endpoint::NodeInfo;
use crate::overlay::ConnectionInfo;

pub type StateRequestSender = Sender<oneshot::Sender<State>>;

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
