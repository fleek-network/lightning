use fleek_crypto::{NodeNetworkingSecretKey, NodeSecretKey, SecretKey};
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub node_key_path: ResolvedPathBuf,
    pub network_key_path: ResolvedPathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            node_key_path: "~/.lightning/keystore/node.pem"
                .try_into()
                .expect("Failed to resolve path."),
            network_key_path: "~/.lightning/keystore/network.pem"
                .try_into()
                .expect("Failed to resolve path."),
        }
    }
}

impl Config {
    pub fn test() -> Self {
        Self {
            node_key_path: "../test-utils/keys/test_node.pem"
                .try_into()
                .expect("Failed to resolve path."),
            network_key_path: "../test-utils/keys/test_network.pem"
                .try_into()
                .expect("Failed to resolve path."),
        }
    }

    pub fn test2() -> Self {
        Self {
            node_key_path: "../test-utils/keys/test_node2.pem"
                .try_into()
                .expect("Failed to resolve path."),
            network_key_path: "../test-utils/keys/test_network2.pem"
                .try_into()
                .expect("Failed to resolve path."),
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
