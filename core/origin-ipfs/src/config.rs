use std::time::Duration;

use cid::Cid;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub gateways: Vec<Gateway>,
    #[serde(with = "humantime_serde")]
    pub gateway_timeout: Duration,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Protocol {
    Http,
    Https,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum RequestFormat {
    CidFirst,
    CidLast,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Gateway {
    pub protocol: Protocol,
    pub authority: String,
    pub request_format: RequestFormat,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            gateways: vec![
                Gateway {
                    protocol: Protocol::Https,
                    authority: "ipfs.w3s.link".to_string(),
                    request_format: RequestFormat::CidFirst,
                },
                Gateway {
                    protocol: Protocol::Https,
                    authority: "cf-ipfs.com".to_string(),
                    request_format: RequestFormat::CidLast,
                },
                Gateway {
                    protocol: Protocol::Https,
                    authority: "fleek.ipfs.io".to_string(),
                    request_format: RequestFormat::CidLast,
                },
                Gateway {
                    protocol: Protocol::Https,
                    authority: "ipfs.io".to_string(),
                    request_format: RequestFormat::CidLast,
                },
                Gateway {
                    protocol: Protocol::Https,
                    authority: "ipfs.runfission.com".to_string(),
                    request_format: RequestFormat::CidLast,
                },
            ],
            gateway_timeout: Duration::from_millis(5000),
        }
    }
}

impl Gateway {
    pub fn build_request(&self, cid: Cid) -> String {
        match self.request_format {
            RequestFormat::CidFirst => {
                format!(
                    "{}://{cid}.{}?format=car",
                    self.protocol.as_str(),
                    self.authority
                )
            },
            RequestFormat::CidLast => {
                format!(
                    "{}://{}/ipfs/{cid}?format=car",
                    self.protocol.as_str(),
                    self.authority
                )
            },
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
