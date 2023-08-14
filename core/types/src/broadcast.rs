use derive_more::IsVariant;
use serde::{Deserialize, Serialize};

/// Numerical value for different gossip topics used by Fleek Network.
// New topics can be added as the system grows.
#[derive(
    Debug, Clone, Copy, PartialEq, PartialOrd, Ord, Eq, Hash, Serialize, Deserialize, IsVariant,
)]
#[repr(u8)]
pub enum Topic {
    /// The gossip topic for
    Consensus = 0x00,
    /// The gossip topic for Fleek Network's indexer DHT.
    DistributedHashTable = 0x01,
    /// The debug topic for tests
    Debug = 0xFF,
}

impl ink_quill::TranscriptBuilderInput for Topic {
    const TYPE: &'static str = "TOPIC";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        let value = *self as u8;
        vec![value]
    }
}
