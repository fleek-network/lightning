use std::net::SocketAddr;
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub max_idle_timeout: Duration,
    pub address: SocketAddr,
    pub http: Option<SocketAddr>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_idle_timeout: Duration::from_millis(30000),
            address: "0.0.0.0:4300".parse().expect("Hardcoded socket address"),
            http: None,
        }
    }
}
