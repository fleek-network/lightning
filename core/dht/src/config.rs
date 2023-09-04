use std::net::SocketAddr;

use fleek_crypto::NodePublicKey;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub address: SocketAddr,
    pub bootstrappers: Vec<Bootstrapper>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Bootstrapper {
    pub address: SocketAddr,
    pub network_public_key: NodePublicKey,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            address: "0.0.0.0:8101".parse().unwrap(),
            bootstrappers: vec![],
        }
    }
}
