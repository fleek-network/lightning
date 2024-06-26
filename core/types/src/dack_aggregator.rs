use serde::{Deserialize, Serialize};

use crate::Blake3Hash;

/// A batch of delivery acknowledgments.
#[derive(Serialize, Deserialize, Debug, Hash)]
pub struct DeliveryAcknowledgmentBatch;

#[derive(
    Serialize, Deserialize, Debug, Hash, Clone, Default, Eq, PartialEq, schemars::JsonSchema,
)]
pub struct DeliveryAcknowledgment {
    /// The service id of the service this was provided through(CDN, compute, ect.)
    pub service_id: u32,
    /// How much of the commodity was served
    pub commodity: u128,
    /// The actual delivery acknowledgment proof
    pub proof: DeliveryAcknowledgmentProof,
    /// Optional metadata field
    pub metadata: Option<Vec<u8>>,
    /// Testnet only
    pub hashes: Vec<Blake3Hash>,
}

#[derive(
    Serialize, Deserialize, Debug, Hash, Clone, Default, Eq, PartialEq, schemars::JsonSchema,
)]
pub struct DeliveryAcknowledgmentProof;
