use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub gateways: Vec<Gateway>,
    pub gateway_timeout: Duration,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Protocol {
    Http,
    Https,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Gateway {
    pub protocol: Protocol,
    pub authority: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            gateways: vec![
                Gateway {
                    protocol: Protocol::Https,
                    authority: "fleek.ipfs.io".to_string(),
                },
                Gateway {
                    protocol: Protocol::Https,
                    authority: "ipfs.io".to_string(),
                },
                Gateway {
                    protocol: Protocol::Https,
                    authority: "ipfs.runfission.com".to_string(),
                },
            ],
            gateway_timeout: Duration::from_millis(5000),
        }
    }
}

impl Protocol {
    pub fn as_str(&self) -> &str {
        match self {
            Protocol::Http => "http",
            Protocol::Https => "https",
        }
    }
}
