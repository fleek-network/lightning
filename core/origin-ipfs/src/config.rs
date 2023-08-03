use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub gateways: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            gateways: vec![
                "gateway.ipfs.io".to_string(),
                "fleek.ipfs.io".to_string(),
                "ipfs.runfission.com".to_string(),
            ],
        }
    }
}
