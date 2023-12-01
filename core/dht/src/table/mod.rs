pub mod bucket;
mod client;
pub mod distance;
mod manager;
pub mod server;

use std::net::SocketAddr;

pub use client::Client;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::types::NodeIndex;
pub use manager::Event;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeInfo {
    // Todo: remove address.
    pub address: SocketAddr,
    pub index: NodeIndex,
    pub key: NodePublicKey,
    pub last_responded: Option<u64>,
}
