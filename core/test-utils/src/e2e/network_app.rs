use std::collections::HashSet;
use std::time::Duration;

use anyhow::Result;
use futures::future::join_all;
use lightning_interfaces::prelude::*;
use lightning_utils::application::QueryRunnerExt;
use lightning_utils::poll::{poll_until, PollUntilError};
use types::{
    Epoch,
    ExecuteTransactionOptions,
    ExecuteTransactionRetry,
    ExecuteTransactionWait,
    UpdateMethod,
};

use super::{BoxedTestNode, TestNetwork};

impl TestNetwork {
    /// Execute epoch change transaction from all nodes and wait for epoch to be incremented.
    ///
    /// Returns an error if the epoch does not change within a timeout.
    pub async fn change_epoch_and_wait_for_complete(&self) -> Result<Epoch> {
        // Execute epoch change transaction from all nodes.
        let new_epoch = self.change_epoch().await?;

        // Wait for epoch to be incremented across all nodes.
        self.wait_for_epoch_change(new_epoch).await?;

        // Return the new epoch.
        Ok(new_epoch)
    }

    /// Execute epoch change transaction from all nodes and return the new epoch.
    ///
    /// This method does not wait for the epoch to be incremented across all nodes, but it does wait
    /// for each of the transactions to be executed.
    pub async fn change_epoch(&self) -> Result<Epoch> {
        self.change_epoch_with_options(Some(ExecuteTransactionOptions {
            wait: ExecuteTransactionWait::Receipt,
            retry: ExecuteTransactionRetry::Default,
            timeout: Some(Duration::from_secs(10)),
        }))
        .await
    }

    pub async fn change_epoch_with_options(
        &self,
        options: Option<ExecuteTransactionOptions>,
    ) -> Result<Epoch> {
        let epoch = self.node(0).app_query().get_current_epoch();
        join_all(self.nodes().map(|node| {
            node.execute_transaction_from_node(UpdateMethod::ChangeEpoch { epoch }, options.clone())
        }))
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
        Ok(epoch + 1)
    }

    /// Wait for the epoch to match the given epoch across all nodes.
    ///
    /// Returns an error if the epoch does not match the given epoch within 20 seconds.
    pub async fn wait_for_epoch_change(&self, new_epoch: Epoch) -> Result<(), PollUntilError> {
        poll_until(
            || async {
                self.nodes()
                    .all(|node| node.app_query().get_current_epoch() == new_epoch)
                    .then_some(())
                    .ok_or(PollUntilError::ConditionNotSatisfied)
            },
            Duration::from_secs(20),
            Duration::from_millis(100),
        )
        .await
    }

    /// Returns the nodes that are part of the current committee.
    ///
    /// This method uses the first node in the network to get the current epoch and committee.
    pub fn committee_nodes(&self) -> Vec<&BoxedTestNode> {
        let node = self.node(0);
        node.app_query()
            .get_committee_members_by_index()
            .into_iter()
            .map(|index| self.node(index))
            .collect()
    }

    /// Returns the nodes that are not part of the current committee.
    ///
    /// This method uses the first node in the network to get the current epoch and committee.
    pub fn non_committee_nodes(&self) -> Vec<&BoxedTestNode> {
        let node = self.node(0);
        let committee_nodes = node
            .app_query()
            .get_committee_members_by_index()
            .into_iter()
            .collect::<HashSet<_>>();
        self.nodes()
            .filter(|node| !committee_nodes.contains(&node.index()))
            .collect()
    }
}
