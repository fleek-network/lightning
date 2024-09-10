use bit_set::BitSet;
use fleek_crypto::{ConsensusAggregateSignature, ConsensusSignature};
use merklize::StateRootHash;
use serde::{Deserialize, Serialize};

use crate::{Epoch, NodeIndex};

/// A checkpoint attestation is a BLS signature over the previous state root, the next state root,
/// and the serialized state digest, as attestation of the state at a given epoch from a node.
#[derive(
    Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash, schemars::JsonSchema,
)]
pub struct CheckpointAttestation {
    pub epoch: Epoch,
    pub node_id: NodeIndex,
    pub previous_state_root: StateRootHash,
    pub next_state_root: StateRootHash,
    pub serialized_state_digest: [u8; 32],
    pub signature: ConsensusSignature,
}

/// An aggregate checkpoint is an aggregate BLS signature over the previous state root, the
/// next state root, and the serialized state digest. This represents a the state root that the
/// a supermajority of the nodes in the network have attested to.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AggregateCheckpoint {
    pub epoch: Epoch,
    pub state_root: StateRootHash,
    pub signature: ConsensusAggregateSignature,
    pub nodes: BitSet,
}

impl schemars::JsonSchema for AggregateCheckpoint {
    fn schema_name() -> String {
        "AggregateCheckpoint".to_string()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed(concat!(module_path!(), "::AggregateCheckpoint"))
    }

    fn json_schema(_gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let sig = Self::default();

        schemars::schema_for_value!(sig).schema.into()
    }
}
