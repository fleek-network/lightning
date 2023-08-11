use fleek_crypto::{NodeNetworkingPublicKey, NodeNetworkingSignature};
use lightning_interfaces::{schema::AutoImplSerde, Blake3Hash, ToDigest, Topic};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BroadcastMessage {
    pub topic: Topic,
    pub originator: NodeNetworkingPublicKey,
    pub payload: Vec<u8>,
}

impl ToDigest for BroadcastMessage {
    fn to_digest(&self) -> [u8; 32] {
        ink_quill::TranscriptBuilder::empty("lightning-broadcast")
            .with("TOPIC", &self.topic)
            .with("PUBKEY", &self.originator.0)
            .with("PAYLOAD", &self.payload)
            .hash()
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
