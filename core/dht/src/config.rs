use std::net::SocketAddr;

use fleek_crypto::NodePublicKey;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub address: SocketAddr,
    pub bootstrappers: Vec<NodePublicKey>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            address: "0.0.0.0:4340".parse().unwrap(),
            bootstrappers: vec![],
        }
    }
}
