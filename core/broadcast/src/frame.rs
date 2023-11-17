use fleek_crypto::NodeSignature;
use ink_quill::{ToDigest, TranscriptBuilder};
use lightning_interfaces::schema::AutoImplSerde;
use lightning_interfaces::types::{Digest, NodeIndex, Topic};
use serde::{Deserialize, Serialize};

pub type MessageInternedId = u16;

#[derive(Debug, Serialize, Deserialize)]
pub struct Want {
    pub interned_id: MessageInternedId,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct Advr {
    pub interned_id: MessageInternedId,
    pub digest: Digest,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub origin: NodeIndex,
    pub signature: NodeSignature,
    pub topic: Topic,
    pub timestamp: u64,
    pub payload: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Frame {
    /// Sent by a single node to advertise a message that they have.
    Advr(Advr),
    /// Sent by the requester of the message to the advertiser indicating
    /// that they want this message.
    Want(Want),
    /// An actual broadcast message.
    Message(Message),
}

impl ToDigest for Message {
    fn transcript(&self) -> ink_quill::TranscriptBuilder {
        TranscriptBuilder::empty("FLEEK_BROADCAST_DOMAIN")
            .with("topic", &self.topic)
            .with("payload", &self.payload)
    }
}

impl AutoImplSerde for Frame {}
