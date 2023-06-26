use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    node_key_path: PathBuf,
    network_key_path: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            node_key_path: "~/.draco/keystore/node.pem".into(),
            network_key_path: "~/.draco/keystore/network.pem".into(),
        }
    }
}
