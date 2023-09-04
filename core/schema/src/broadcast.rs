use fleek_crypto::{NodePublicKey, NodeSignature};
use ink_quill::{ToDigest, TranscriptBuilder};
use lightning_types::{ImmutablePointer, NodeIndex, Topic};
use serde::{Deserialize, Serialize};

use crate::AutoImplSerde;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BroadcastMessage {
    pub topic: Topic,
    pub originator: NodePublicKey,
    pub payload: Vec<u8>,
}

impl ToDigest for BroadcastMessage {
    fn transcript(&self) -> TranscriptBuilder {
        TranscriptBuilder::empty("lightning-broadcast")
            .with("TOPIC", &self.topic)
            .with("PUBKEY", &self.originator.0)
            .with("PAYLOAD", &self.payload)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BroadcastFrame {
    Advertise {
        digest: [u8; 32],
    },
    Want {
        digest: [u8; 32],
    },
    Message {
        message: BroadcastMessage,
        signature: NodeSignature,
    },
}
impl AutoImplSerde for BroadcastFrame {}

/// Once a content is put on the network (i.e a node fetches the content from the origin), the
/// node that fetched the content computes the blake3 hash of the content and signs a record
/// attesting that it witnessed the immutable pointer resolving to the said blake3 hash.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResolvedImmutablePointerRecord {
    /// The immutable pointer that was fetched.
    pub pointer: ImmutablePointer,
    /// The blake3 hash of the content. Used to store the content on the blockstore.
    pub hash: [u8; 32],
    /// The public key of the node which fetched and attested to this content.
    // TODO: Replace with node id later.
    pub originator: NodeIndex,
    /// The signature of the node.
    pub signature: NodeSignature,
}

impl AutoImplSerde for ResolvedImmutablePointerRecord {}
