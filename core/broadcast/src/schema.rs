use fleek_crypto::{NodeNetworkingPublicKey, NodeNetworkingSignature};
use ink_quill::TranscriptBuilder;
use lightning_interfaces::{schema::AutoImplSerde, types::Topic, Blake3Hash, ToDigest};
use serde::{Deserialize, Serialize};

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
        digest: Blake3Hash,
    },
    Want {
        digest: Blake3Hash,
    },
    Message {
        message: BroadcastMessage,
        signature: NodeNetworkingSignature,
    },
}
impl AutoImplSerde for BroadcastFrame {}
