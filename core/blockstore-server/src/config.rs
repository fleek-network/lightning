use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub address: SocketAddr,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            address: ([0, 0, 0, 0], 4211).into(),
        }
    }
}
