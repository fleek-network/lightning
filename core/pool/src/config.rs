use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    // TODO(qti3e): Should be a duration.
    pub max_idle_timeout: u64,
    pub address: SocketAddr,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_idle_timeout: 300, // 5 minutes.
            address: "0.0.0.0:4200".parse().expect("Hardcoded socket address"),
        }
    }
}
