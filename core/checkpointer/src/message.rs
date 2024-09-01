use lightning_interfaces::schema::AutoImplSerde;
use lightning_interfaces::types::CheckpointHeader;
use serde::{Deserialize, Serialize};

/// The message envelope that is broadcasted to all nodes in the network on the checkpointer topic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum CheckpointBroadcastMessage {
    CheckpointHeader(CheckpointHeader),
}

impl AutoImplSerde for CheckpointBroadcastMessage {}
