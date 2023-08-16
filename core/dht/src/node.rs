use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use fleek_crypto::NodePublicKey;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeInfo {
    pub address: SocketAddr,
    pub key: NodePublicKey,
}
