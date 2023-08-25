use std::net::SocketAddr;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct HandshakeServerConfig {
    pub websocket_address: SocketAddr,
}

impl Default for HandshakeServerConfig {
    fn default() -> Self {
        Self {
            websocket_address: SocketAddr::from_str("0.0.0.0:4202").unwrap(),
        }
    }
}
