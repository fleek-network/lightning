use std::net::IpAddr;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    /// Address to bind to
    pub addr: IpAddr,
    /// Port to listen on
    pub port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            addr: "0.0.0.0".parse().unwrap(),
            port: 8545,
        }
    }
}
