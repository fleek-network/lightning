use bit_set::BitSet;
use fleek_crypto::{ConsensusAggregateSignature, ConsensusSignature};
use serde::{Deserialize, Serialize};

use crate::{Epoch, NodeIndex};

/// A checkpoint header is a BLS signature over the previous state root, the next state root, and
/// the serialized state digest, as attestation of the state at a given epoch from a node.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CheckpointHeader {
    pub epoch: Epoch,
    pub node_id: NodeIndex,
    pub previous_state_root: [u8; 32],
    pub next_state_root: [u8; 32],
    pub serialized_state_digest: [u8; 32],
    pub signature: ConsensusSignature,
}

/// An aggregate checkpoint header is an aggregate BLS signature over the previous state root, the
/// next state root, and the serialized state digest. This represents a the state root that the
/// a supermajority of the nodes in the network have attested to.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AggregateCheckpointHeader {
    pub epoch: Epoch,
    pub previous_state_root: [u8; 32],
    pub next_state_root: [u8; 32],
    pub signature: ConsensusAggregateSignature,
    pub nodes: BitSet,
}
