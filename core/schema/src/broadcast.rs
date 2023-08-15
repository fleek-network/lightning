use fleek_crypto::{NodeNetworkingPublicKey, NodeNetworkingSignature};
use ink_quill::{ToDigest, TranscriptBuilder};
use lightning_types::Topic;
use serde::{Deserialize, Serialize};

use crate::AutoImplSerde;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BroadcastMessage {
    pub topic: Topic,
    pub originator: NodeNetworkingPublicKey,
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
        signature: NodeNetworkingSignature,
    },
}
impl AutoImplSerde for BroadcastFrame {}
