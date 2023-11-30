use std::net::SocketAddr;

use fleek_crypto::NodePublicKey;
use lightning_interfaces::types::NodeIndex;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeInfo {
    // Todo: remove address.
    pub address: SocketAddr,
    pub index: NodeIndex,
    pub key: NodePublicKey,
    pub last_responded: Option<u64>,
}
