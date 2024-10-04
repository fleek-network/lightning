use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures::future::join_all;
use lightning_interfaces::types::Genesis;
use lightning_interfaces::{ApplicationInterface, PoolInterface};
use lightning_utils::poll::{poll_until, PollUntilError};
use ready::ReadyWaiter;
use tempfile::tempdir;

use super::{TestGenesisBuilder, TestNetwork, TestNode, TestNodeBuilder};
use crate::consensus::{Config as MockConsensusConfig, MockConsensusGroup};

pub type GenesisMutator = Arc<dyn Fn(&mut Genesis)>;

#[derive(Clone)]
pub struct TestNetworkBuilder {
    pub num_nodes: u32,
    pub genesis_mutator: Option<GenesisMutator>,
    pub mock_consensus_config: Option<MockConsensusConfig>,
}

impl TestNetworkBuilder {
    pub fn new() -> Self {
        Self {
            num_nodes: 3,
            genesis_mutator: None,
            mock_consensus_config: Some(MockConsensusConfig {
                max_ordering_time: 1,
                min_ordering_time: 0,
                probability_txn_lost: 0.0,
                new_block_interval: Duration::from_secs(0),
                ..Default::default()
            }),
        }
    }

    pub fn with_num_nodes(mut self, num_nodes: u32) -> Self {
        self.num_nodes = num_nodes;
        self
    }

    pub fn with_genesis_mutator<F>(mut self, mutator: F) -> Self
    where
        F: Fn(&mut Genesis) + 'static,
    {
        self.genesis_mutator = Some(Arc::new(mutator));
        self
    }

    pub fn with_mock_consensus(mut self, config: MockConsensusConfig) -> Self {
        self.mock_consensus_config = Some(config);
        self
    }

    pub fn without_mock_consensus(mut self) -> Self {
        self.mock_consensus_config = None;
        self
    }

    /// Builds a new test network with the given number of nodes, and starts each of them.
    pub async fn build(self) -> Result<TestNetwork> {
        let temp_dir = tempdir()?;

        // Configure mock consensus if enabled.
        let (consensus_group, consensus_group_start) =
            if let Some(config) = &self.mock_consensus_config {
                // Build the shared mock consensus group.
                let consensus_group_start = Arc::new(tokio::sync::Notify::new());
                let consensus_group =
                    MockConsensusGroup::new(config.clone(), Some(consensus_group_start.clone()));
                (Some(consensus_group), Some(consensus_group_start))
            } else {
                (None, None)
            };

        // Build and start the nodes.
        let mut nodes = join_all((0..self.num_nodes).map(|i| {
            let mut builder = TestNodeBuilder::new(temp_dir.path().join(format!("node-{}", i)));
            if let Some(consensus_group) = &consensus_group {
                builder = builder.with_mock_consensus(Some(consensus_group.clone()));
            }
            builder.build()
        }))
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

        // Wait for ready before building genesis.
        join_all(
            nodes
                .iter_mut()
                .map(|node| node.before_genesis_ready.wait()),
        )
        .await;

        // Build genesis.
        let genesis = {
            let mut builder = TestGenesisBuilder::default();
            if let Some(mutator) = self.genesis_mutator.clone() {
                builder = builder.with_mutator(mutator);
            }
            for node in nodes.iter() {
                builder = builder.with_node(node);
            }
            builder.build()
        };

        // Apply genesis on each node.
        join_all(
            nodes
                .iter_mut()
                .map(|node| node.app.apply_genesis(genesis.clone())),
        )
        .await;

        // Wait for the pool to establish all of the node connections.
        self.wait_for_connected_peers(&nodes).await?;

        // Wait for ready after genesis.
        join_all(nodes.iter_mut().map(|node| node.after_genesis_ready.wait())).await;

        // Notify the shared mock consensus group that it can start.
        if let Some(consensus_group_start) = &consensus_group_start {
            consensus_group_start.notify_waiters();
        }

        let network = TestNetwork::new(temp_dir, genesis, nodes).await?;
        Ok(network)
    }

    pub async fn wait_for_connected_peers(&self, nodes: &[TestNode]) -> Result<(), PollUntilError> {
        poll_until(
            || async {
                let peers_by_node = join_all(nodes.iter().map(|node| node.pool.connected_peers()))
                    .await
                    .into_iter()
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| PollUntilError::ConditionError(e.to_string()))?;

                peers_by_node
                    .iter()
                    .all(|peers| peers.len() == nodes.len() - 1)
                    .then_some(())
                    .ok_or(PollUntilError::ConditionNotSatisfied)
            },
            Duration::from_secs(3),
            Duration::from_millis(200),
        )
        .await
    }
}

impl Default for TestNetworkBuilder {
    fn default() -> Self {
        Self::new()
    }
}
