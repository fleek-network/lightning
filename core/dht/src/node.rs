use std::net::SocketAddr;

use fleek_crypto::NodePublicKey;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeInfo {
    pub address: SocketAddr,
    pub key: NodePublicKey,
}
