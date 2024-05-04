use std::net::SocketAddr;
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub address: SocketAddr,
    // /// The number of times that we ping each peer per epoch.
    // pub num_pings_per_peer: u16,
    /// The interval for sending pings.
    pub ping_interval: Duration,
    /// The duration after which a ping will be reported as unanswered
    pub timeout: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            address: "0.0.0.0:4350".parse().unwrap(),
            //num_pings_per_peer: 3,
            ping_interval: Duration::from_secs(5),
            timeout: Duration::from_secs(15),
        }
    }
}
