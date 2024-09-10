use lightning_interfaces::schema::AutoImplSerde;
use lightning_interfaces::types::CheckpointAttestation;
use serde::{Deserialize, Serialize};

/// The message envelope that is broadcasted to all nodes in the network on the checkpointer topic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CheckpointBroadcastMessage {
    CheckpointAttestation(CheckpointAttestation),
}

impl AutoImplSerde for CheckpointBroadcastMessage {}
