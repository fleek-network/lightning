use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub gateways: Vec<Gateway>,
}

#[derive(Serialize, Deserialize)]
pub enum Protocol {
    Http,
    Https,
}

#[derive(Serialize, Deserialize)]
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
                    authority: "gateway.ipfs.io".to_string(),
                },
                Gateway {
                    protocol: Protocol::Https,
                    authority: "fleek.ipfs.io".to_string(),
                },
                Gateway {
                    protocol: Protocol::Https,
                    authority: "ipfs.runfission.com".to_string(),
                },
            ],
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
