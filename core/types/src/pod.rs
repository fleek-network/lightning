use serde::{Deserialize, Serialize};

/// A batch of delivery acknowledgments.
#[derive(Serialize, Deserialize, Debug, Hash)]
pub struct DeliveryAcknowledgmentBatch;

#[derive(Serialize, Deserialize, Debug, Hash, Clone, Default, Eq, PartialEq)]
pub struct DeliveryAcknowledgment;
