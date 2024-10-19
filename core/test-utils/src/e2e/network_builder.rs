use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures::future::join_all;
use lightning_application::state::QueryRunner;
use lightning_committee_beacon::CommitteeBeaconConfig;
use lightning_interfaces::types::{Genesis, Staking};
use lightning_interfaces::{ApplicationInterface, NodeComponents};
use lightning_utils::poll::{poll_until, PollUntilError};
use tempfile::tempdir;

use super::{
    try_init_tracing,
    BoxedTestNode,
    GenesisMutator,
    TestGenesisBuilder,
    TestGenesisNodeBuilder,
    TestNetwork,
    TestNodeBuilder,
};
use crate::consensus::{MockConsensusConfig, MockConsensusGroup};

pub struct TestNetworkBuilder {
    pub nodes: Vec<BoxedTestNode>,
    pub generated_committee_nodes: u8,
    pub generated_non_committee_nodes: u8,
    pub genesis_mutator: Option<GenesisMutator>,
    pub mock_consensus_config: Option<MockConsensusConfig>,
    pub mock_consensus_group: Option<MockConsensusGroup>,
    pub committee_beacon_config: Option<CommitteeBeaconConfig>,
}

impl TestNetworkBuilder {
    pub fn new() -> Self {
        Self {
            nodes: vec![],
            generated_committee_nodes: 0,
            generated_non_committee_nodes: 0,
            genesis_mutator: None,
            mock_consensus_config: None,
            mock_consensus_group: None,
            committee_beacon_config: None,
        }
        .with_mock_consensus(MockConsensusConfig {
            max_ordering_time: 1,
            min_ordering_time: 0,
            probability_txn_lost: 0.0,
            new_block_interval: Duration::from_secs(0),
            transactions_to_lose: Default::default(),
            block_buffering_interval: Duration::from_millis(0),
        })
    }

    pub fn with_node(mut self, node: BoxedTestNode) -> Self {
        self.nodes.push(node);
        self
    }

    pub async fn with_committee_nodes<C: NodeComponents>(mut self, num_nodes: usize) -> Self
    where
        C::ApplicationInterface: ApplicationInterface<C, SyncExecutor = QueryRunner>,
    {
        for _ in 0..num_nodes {
            let mut builder = TestNodeBuilder::new().with_is_genesis_committee(true);
            if let Some(consensus_group) = &self.mock_consensus_group {
                builder = builder.with_mock_consensus(consensus_group.clone());
            }
            if let Some(committee_beacon_config) = &self.committee_beacon_config {
                builder = builder.with_committee_beacon_config(committee_beacon_config.clone());
            }
            let node = builder.build::<C>().await.unwrap();
            self.nodes.push(node);
        }
        self
    }

    pub async fn with_non_committee_nodes<C: NodeComponents>(mut self, num_nodes: usize) -> Self
    where
        C::ApplicationInterface: ApplicationInterface<C, SyncExecutor = QueryRunner>,
    {
        for _ in 0..num_nodes {
            let mut builder = TestNodeBuilder::new().with_is_genesis_committee(false);
            if let Some(consensus_group) = &self.mock_consensus_group {
                builder = builder.with_mock_consensus(consensus_group.clone());
            }
            if let Some(committee_beacon_config) = &self.committee_beacon_config {
                builder = builder.with_committee_beacon_config(committee_beacon_config.clone());
            }
            let node = builder.build::<C>().await.unwrap();
            self.nodes.push(node);
        }
        self
    }

    pub fn with_genesis_mutator<F>(mut self, mutator: F) -> Self
    where
        F: Fn(&mut Genesis) + 'static,
    {
        self.genesis_mutator = Some(Arc::new(mutator));
        self
    }

    pub fn with_committee_beacon_config(mut self, config: CommitteeBeaconConfig) -> Self {
        self.committee_beacon_config = Some(config);
        self
    }

    /// Sets up a mock consensus group with the given config.
    ///
    /// This will overwrite any existing mock consensus group, and will not configure existing nodes
    /// to use this consensus group since they have already been built, so this should be called
    /// before building and adding any nodes.
    pub fn with_mock_consensus(mut self, config: MockConsensusConfig) -> Self {
        self.mock_consensus_config = Some(config.clone());

        let notify_start = Arc::new(tokio::sync::Notify::new());
        let consensus_group = MockConsensusGroup::new::<QueryRunner>(
            config.clone(),
            None,
            Some(notify_start.clone()),
        );
        self.mock_consensus_group = Some(consensus_group);

        self
    }

    pub fn mock_consensus_group(&self) -> MockConsensusGroup {
        self.mock_consensus_group
            .clone()
            .expect("mock consensus group not set")
    }

    /// Builds a new test network with the given number of nodes, and starts each of them.
    pub async fn build(mut self) -> Result<TestNetwork> {
        let _ = try_init_tracing();

        let temp_dir = tempdir()?;

        // Start the nodes.
        join_all(self.nodes.iter_mut().map(|node| node.start())).await;

        // Build genesis.
        let genesis = {
            let mut builder = TestGenesisBuilder::default();
            if let Some(mutator) = self.genesis_mutator.clone() {
                builder = builder.with_mutator(mutator);
            }
            for node in &self.nodes {
                builder = builder.with_node(
                    TestGenesisNodeBuilder::new()
                        .with_owner(node.get_owner_public_key().into())
                        .with_node_secret_key(node.get_node_secret_key())
                        .with_consensus_secret_key(node.get_consensus_secret_key())
                        .with_node_ports(node.get_node_ports().await.unwrap())
                        .with_stake(Staking {
                            staked: 1000u32.into(),
                            stake_locked_until: 0,
                            locked: 0u32.into(),
                            locked_until: 0,
                        })
                        .with_is_committee(node.is_genesis_committee())
                        .build(),
                )
            }
            builder.build()
        };

        // Apply genesis on each node.
        join_all(
            self.nodes
                .iter()
                .map(|node| node.apply_genesis(genesis.clone())),
        )
        .await;

        // Wait for the pool to establish all of the node connections.
        self.wait_for_connected_peers(&self.nodes).await?;

        // Notify the shared mock consensus group that it can start.
        if let Some(consensus_group) = &self.mock_consensus_group {
            consensus_group.start();
        }

        let network = TestNetwork::new(temp_dir, genesis, self.nodes).await?;
        Ok(network)
    }

    pub async fn wait_for_connected_peers(
        &self,
        nodes: &[BoxedTestNode],
    ) -> Result<(), PollUntilError> {
        poll_until(
            || async {
                let peers_by_node = join_all(nodes.iter().map(|node| node.pool_connected_peers()))
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
