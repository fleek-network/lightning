use lightning_interfaces::types::Epoch;
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
            node.emit_epoch_changed_notification(
                epoch,
                last_epoch_hash,
                previous_state_root,
                new_state_root,
            );
        }
    }
}
