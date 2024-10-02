use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{Epoch, NodeIndex};
use merklize::StateRootHash;

use super::TestNetwork;

impl TestNetwork {
    /// Emit an epoch changed notification to all nodes.
    pub async fn notify_epoch_changed(
        &self,
        epoch: Epoch,
        previous_state_root: StateRootHash,
        new_state_root: StateRootHash,
        last_epoch_hash: [u8; 32],
    ) {
        for node in self.nodes() {
            self.notify_node_epoch_changed(
                node.index(),
                epoch,
                last_epoch_hash,
                previous_state_root,
                new_state_root,
            )
            .await;
        }
    }

    /// Emit an epoch change notification to a specific node.
    pub async fn notify_node_epoch_changed(
        &self,
        node_id: NodeIndex,
        epoch: Epoch,
        last_epoch_hash: [u8; 32],
        previous_state_root: StateRootHash,
        new_state_root: StateRootHash,
    ) {
        self.node(node_id).notifier.get_emitter().epoch_changed(
            epoch,
            last_epoch_hash,
            previous_state_root,
            new_state_root,
        );
    }
}
