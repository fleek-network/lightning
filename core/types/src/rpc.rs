use fleek_crypto::{ConsensusPublicKey, NodePublicKey};
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug, schemars::JsonSchema)]
pub struct PublicKeys {
    pub node_public_key: NodePublicKey,
    pub consensus_public_key: ConsensusPublicKey,
}
