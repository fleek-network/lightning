use std::time::Duration;

use anyhow::Result;
use futures::future::join_all;
use lightning_interfaces::prelude::*;
use lightning_utils::poll::{poll_until, PollUntilError};
use types::{Epoch, Metadata, UpdateMethod};

use super::{TestNetwork, TestNode};

impl TestNetwork {
    /// Execute epoch change transaction from all nodes and wait for epoch to be incremented.
    pub async fn change_epoch_and_wait_for_complete(&self) -> Result<Epoch, PollUntilError> {
        // Execute epoch change transaction from all nodes.
        let new_epoch = self.change_epoch().await;

        // Wait for epoch to be incremented across all nodes.
        self.wait_for_epoch_change(new_epoch).await?;

        // Return the new epoch.
        Ok(new_epoch)
    }

    pub async fn change_epoch(&self) -> Epoch {
        let epoch = self.node(0).get_epoch();
        join_all(
            self.nodes().map(|node| {
                node.execute_transaction_from_node(UpdateMethod::ChangeEpoch { epoch })
            }),
        )
        .await;
        epoch + 1
    }

    pub async fn wait_for_epoch_change(&self, new_epoch: Epoch) -> Result<(), PollUntilError> {
        poll_until(
            || async {
                self.nodes()
                    .all(|node| node.get_epoch() == new_epoch)
                    .then_some(())
                    .ok_or(PollUntilError::ConditionNotSatisfied)
            },
            Duration::from_secs(5),
            Duration::from_millis(100),
        )
        .await
    }

    pub fn committee_nodes(&self) -> Vec<&TestNode> {
        let node = self.node(0);
        let epoch = node.get_epoch();
        node.app_query
            .get_committee_info(&epoch, |committee| committee.members)
            .unwrap_or_default()
            .into_iter()
            .map(|index| self.node(index))
            .collect()
    }

    pub fn non_committee_nodes(&self) -> Vec<&TestNode> {
        let node = self.node(0);
        let epoch = node.get_epoch();
        let committee_nodes = node
            .app_query
            .get_committee_info(&epoch, |committee| committee.members)
            .unwrap_or_default();
        self.nodes()
            .filter(|node| !committee_nodes.contains(&node.index()))
            .collect()
    }

    pub fn get_epoch(&self) -> Epoch {
        self.node(0).get_epoch()
    }

    /// Wait for committee selection beacon phase to be unset.
    pub async fn wait_for_committee_selection_beacon_phase_unset(
        &self,
    ) -> Result<(), PollUntilError> {
        poll_until(
            || async {
                self.node(0)
                    .app
                    .sync_query()
                    .get_metadata(&Metadata::CommitteeSelectionBeaconPhase)
                    .is_none()
                    .then_some(())
                    .ok_or(PollUntilError::ConditionNotSatisfied)
            },
            Duration::from_secs(30),
            Duration::from_millis(100),
        )
        .await
    }
}
