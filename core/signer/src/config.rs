use std::path::PathBuf;

use fleek_crypto::{NodeNetworkingSecretKey, NodeSecretKey, SecretKey};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub node_key_path: PathBuf,
    pub network_key_path: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            node_key_path: "~/.freek/keystore/node.pem".into(),
            network_key_path: "~/.freek/keystore/network.pem".into(),
        }
    }
}

impl Config {
    pub fn test() -> Self {
        Self {
            node_key_path: "../test-utils/keys/test_node.pem".into(),
            network_key_path: "../test-utils/keys/test_network.pem".into(),
        }
    }

    pub fn test2() -> Self {
        Self {
            node_key_path: "../test-utils/keys/test_node2.pem".into(),
            network_key_path: "../test-utils/keys/test_network2.pem".into(),
        }
    }

    pub fn load_test_keys(&self) -> (NodeSecretKey, NodeNetworkingSecretKey) {
        let encoded_node = std::fs::read_to_string(self.node_key_path.clone())
            .expect("Failed to read node pem file");

        let encoded_network = std::fs::read_to_string(self.network_key_path.clone())
            .expect("Failed to read network pem file");

        (
            NodeSecretKey::decode_pem(&encoded_node).expect("Failed to decode node pem file"),
            NodeNetworkingSecretKey::decode_pem(&encoded_network)
                .expect("Failed to decode network pem file"),
        )
    }
}
