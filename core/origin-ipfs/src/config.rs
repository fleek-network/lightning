use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub gateways: Vec<Gateway>,
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
                    authority: "ipfs.io".to_string(),
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
