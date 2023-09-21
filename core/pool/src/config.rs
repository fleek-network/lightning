use std::net::SocketAddr;
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub keep_alive_interval: Duration,
    pub address: SocketAddr,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            keep_alive_interval: Duration::from_secs(8),
            address: "0.0.0.0:4200".parse().expect("Hardcoded socket address"),
        }
    }
}
