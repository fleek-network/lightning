use fleek_crypto::{ConsensusSecretKey, NodeSecretKey, SecretKey};
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub node_key_path: ResolvedPathBuf,
    pub consensus_key_path: ResolvedPathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            node_key_path: "~/.lightning/keystore/node.pem"
                .try_into()
                .expect("Failed to resolve path."),
            consensus_key_path: "~/.lightning/keystore/consensus.pem"
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
            consensus_key_path: "../test-utils/keys/test_consensus.pem"
                .try_into()
                .expect("Failed to resolve path."),
        }
    }

    pub fn test2() -> Self {
        Self {
            node_key_path: "../test-utils/keys/test_node2.pem"
                .try_into()
                .expect("Failed to resolve path."),
            consensus_key_path: "../test-utils/keys/test_consensus2.pem"
                .try_into()
                .expect("Failed to resolve path."),
        }
    }

    pub fn load_test_keys(&self) -> (ConsensusSecretKey, NodeSecretKey) {
        let encoded_node = std::fs::read_to_string(self.node_key_path.clone())
            .expect("Failed to read node pem file");

        let encoded_consensus = std::fs::read_to_string(self.consensus_key_path.clone())
            .expect("Failed to read consensus pem file");

        (
            ConsensusSecretKey::decode_pem(&encoded_consensus)
                .expect("Failed to decode consensus pem file"),
            NodeSecretKey::decode_pem(&encoded_node).expect("Failed to decode node pem file"),
        )
    }
}
