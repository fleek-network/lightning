use std::net::SocketAddr;
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct WebTransportConfig {
    pub address: SocketAddr,
    pub keep_alive: Option<Duration>,
}

impl Default for WebTransportConfig {
    fn default() -> Self {
        Self {
            address: ([0, 0, 0, 0], 4321).into(),
            keep_alive: None,
        }
    }
}
