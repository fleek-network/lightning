use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures::future::join_all;
use lightning_application::config::StorageConfig;
use lightning_application::state::QueryRunner;
use lightning_application::{Application, ApplicationConfig};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::UpdateMethod;
use lightning_interfaces::{
    CommitteeBeaconInterface,
    CommitteeBeaconQueryInterface,
    SyncQueryRunnerInterface,
};
use lightning_node::Node;
use lightning_notifier::Notifier;
use lightning_signer::Signer;
use lightning_test_utils::consensus::{
    Config as MockConsensusConfig,
    MockConsensus,
    MockConsensusGroup,
    MockForwarder,
};
use lightning_test_utils::e2e::{
    try_init_tracing,
    DowncastToTestFullNode,
    TestFullNodeComponentsWithMockConsensus,
    TestGenesisBuilder,
    TestGenesisNodeBuilder,
    TestNetworkBuilder,
};
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_utils::application::QueryRunnerExt;
use lightning_utils::poll::{poll_until, PollUntilError};
use lightning_utils::transaction::{TransactionClient, TransactionSigner};
use tempfile::{tempdir, TempDir};
use tokio::time::Instant;
use types::{
    CommitteeSelectionBeaconPhase,
    Epoch,
    ExecuteTransactionOptions,
    ExecuteTransactionWait,
    Genesis,
};

use crate::{
    CommitteeBeaconComponent,
    CommitteeBeaconConfig,
    CommitteeBeaconDatabaseConfig,
    CommitteeBeaconTimerConfig,
};

#[tokio::test]
async fn test_start_shutdown() {
    let node = lightning_test_utils::e2e::TestNodeBuilder::new()
        .build::<TestFullNodeComponentsWithMockConsensus>()
        .await
        .unwrap();
    node.shutdown().await;
}

#[tokio::test]
async fn test_epoch_change_single_node() {
    let mut network = TestNetworkBuilder::new()
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(1)
        .await
        .build()
        .await
        .unwrap();
    let node = network
        .node(0)
        .downcast::<TestFullNodeComponentsWithMockConsensus>();

    // Send epoch change transaction from all nodes.
    let epoch = network.change_epoch().await;

    // Check that beacon phase is set.
    // We don't check for commit phase specifically because we can't be sure it hasn't transitioned
    // to the reveal phase before checking.
    poll_until(
        || async {
            node.get_committee_selection_beacon_phase()
                .is_some()
                .then_some(())
                .ok_or(PollUntilError::ConditionNotSatisfied)
        },
        Duration::from_secs(3),
        Duration::from_millis(100),
    )
    .await
    .unwrap();

    // Check that beacons are in app state.
    // These difficult to catch this at the right time with queries, so we just check that the
    // number is less than or equal to the number of nodes.
    let beacons = node.app_query().get_committee_selection_beacons();
    assert!(beacons.len() <= network.node_count());

    // Check that beacons are in local database.
    // These difficult to catch this at the right time with queries, so we just check that the
    // number is less than or equal to the number of nodes.
    let beacons = node.committee_beacon().query().get_beacons();
    assert!(beacons.len() <= network.node_count());

    // Wait for reveal phase to complete and beacon phase to be unset.
    network
        .wait_for_committee_selection_beacon_phase_unset()
        .await
        .unwrap();

    // Check that the epoch has been incremented.
    let new_epoch = node.get_epoch();
    assert_eq!(new_epoch, epoch);

    // Check that there are no node beacons (commits and reveals) in app state.
    let beacons = node.app_query().get_committee_selection_beacons();
    assert!(beacons.is_empty());

    // Clearing the beacons at epoch change is best-effort, since we can't guarantee that
    // the notification will be received or the listener will be running, in the case of a
    // deployment for example. This is fine, since the beacons will be cleared on the next
    // committee selection phase anyway, and we don't rely on it for correctness.
    let beacons = node.committee_beacon().query().get_beacons();
    assert!(beacons.len() <= network.node_count());

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_epoch_change_multiple_nodes() {
    let mut network = TestNetworkBuilder::new()
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(3)
        .await
        .build()
        .await
        .unwrap();
    let node = network
        .node(0)
        .downcast::<TestFullNodeComponentsWithMockConsensus>();

    // Send epoch change transaction from all nodes.
    let epoch = network.change_epoch().await;

    // Check that beacon phase is set.
    // We don't check for commit phase specifically because we can't be sure it hasn't transitioned
    // to the reveal phase before checking.
    poll_until(
        || async {
            node.get_committee_selection_beacon_phase()
                .is_some()
                .then_some(())
                .ok_or(PollUntilError::ConditionNotSatisfied)
        },
        Duration::from_secs(3),
        Duration::from_millis(100),
    )
    .await
    .unwrap();

    // Check that beacons are in app state.
    // It's difficult to catch this at the right time with queries, so we just check that the
    // number is less than or equal to the number of nodes.
    let beacons = node.app_query().get_committee_selection_beacons();
    assert!(beacons.len() <= network.node_count());

    // Check that beacons are in local database.
    // It's difficult to catch this at the right time with queries, so we just check that the
    // number is less than or equal to the number of nodes.
    let beacons = node.committee_beacon().query().get_beacons();
    assert!(beacons.len() <= network.node_count());

    // Wait for reveal phase to complete and beacon phase to be unset.
    network
        .wait_for_committee_selection_beacon_phase_unset()
        .await
        .unwrap();

    // Check that the epoch has been incremented.
    let new_epoch = node.get_epoch();
    assert_eq!(new_epoch, epoch);

    // Check that there are no node beacons (commits and reveals) in app state.
    let beacons = node.app_query().get_committee_selection_beacons();
    assert!(beacons.is_empty());

    // Clearing the beacons at epoch change is best-effort, since we can't guarantee that
    // the notification will be received or the listener will be running, in the case of a
    // deployment for example. This is fine, since the beacons will be cleared on the next
    // committee selection phase anyway, and we don't rely on it for correctness.
    let beacons = node.committee_beacon().query().get_beacons();
    assert!(beacons.len() <= network.node_count());

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_block_executed_in_waiting_phase_should_do_nothing() {
    let mut network = TestNetworkBuilder::new()
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(2)
        .await
        .build()
        .await
        .unwrap();
    let node = network
        .node(0)
        .downcast::<TestFullNodeComponentsWithMockConsensus>();
    let query = node.app_query();

    // Check beacon phase before submitting transaction.
    let phase = query.get_committee_selection_beacon_phase();
    assert!(phase.is_none());

    // Submit a transaction that does nothing except increment the node's nonce.
    network
        .node(0)
        .execute_transaction_from_node(UpdateMethod::IncrementNonce {}, None)
        .await
        .unwrap();

    // Check that beacon phase has not changed.
    let phase = query.get_committee_selection_beacon_phase();
    assert!(phase.is_none());

    // Check that there are no node beacons (commits and reveals) in app state.
    let beacons = query.get_committee_selection_beacons();
    assert!(beacons.is_empty());

    // Check that there are no beacons in our local database.
    let beacons = node.committee_beacon().query().get_beacons();
    assert!(beacons.is_empty());

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_insufficient_participation_in_commit_phase() {
    // TODO(snormore): DRY this up.
    let consensus_group_start = Arc::new(tokio::sync::Notify::new());
    let consensus_group = MockConsensusGroup::new::<QueryRunner>(
        MockConsensusConfig {
            min_ordering_time: 0,
            max_ordering_time: 0,
            probability_txn_lost: 0.0,
            transactions_to_lose: HashSet::new(),
            new_block_interval: Duration::from_secs(0),
        },
        None,
        Some(consensus_group_start.clone()),
    );

    let mut nodes = vec![
        TestNodeBuilder::new()
            .with_consensus_group(consensus_group.clone())
            .build::<TestNodeComponents>()
            .await
            .unwrap(),
    ];
    let mut mocked_nodes = vec![
        TestNodeBuilder::new()
            .with_consensus_group(consensus_group.clone())
            .build::<TestNodeComponentsWithMockCommitteeBeacon>()
            .await
            .unwrap(),
        TestNodeBuilder::new()
            .with_consensus_group(consensus_group.clone())
            .build::<TestNodeComponentsWithMockCommitteeBeacon>()
            .await
            .unwrap(),
    ];

    // Start the nodes.
    join_all(nodes.iter_mut().map(|node| node.start())).await;
    join_all(mocked_nodes.iter_mut().map(|node| node.start())).await;

    // Build genesis from nodes.
    let mut builder = TestGenesisBuilder::new();
    for node in nodes.iter() {
        builder = builder.with_node(
            TestGenesisNodeBuilder::new()
                .with_node_secret_key(node.keystore.get_ed25519_sk())
                .with_consensus_secret_key(node.keystore.get_bls_sk())
                .build(),
        );
    }
    for node in mocked_nodes.iter_mut() {
        builder = builder.with_node(
            TestGenesisNodeBuilder::new()
                .with_node_secret_key(node.keystore.get_ed25519_sk())
                .with_consensus_secret_key(node.keystore.get_bls_sk())
                .build(),
        );
    }
    let genesis = builder
        .with_mutator(Arc::new(|genesis: &mut Genesis| {
            genesis.committee_selection_beacon_commit_phase_duration = 2;
            genesis.committee_selection_beacon_reveal_phase_duration = 2;
        }))
        .build();

    // Apply genesis to all nodes.
    join_all(
        nodes
            .iter()
            .map(|node| node.app.apply_genesis(genesis.clone())),
    )
    .await
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .unwrap();
    join_all(
        mocked_nodes
            .iter()
            .map(|node| node.app.apply_genesis(genesis.clone())),
    )
    .await
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .unwrap();
    consensus_group_start.notify_one();

    // Execute epoch change transactions from all nodes.
    let epoch = 0;
    join_all(nodes.iter().map(|node| async {
        node.node_transaction_client()
            .await
            .execute_transaction(
                UpdateMethod::ChangeEpoch { epoch },
                Some(ExecuteTransactionOptions {
                    wait: ExecuteTransactionWait::Receipt,
                    ..Default::default()
                }),
            )
            .await
    }))
    .await
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .unwrap();
    join_all(mocked_nodes.iter().map(|node| async {
        node.node_transaction_client()
            .await
            .execute_transaction(
                UpdateMethod::ChangeEpoch { epoch },
                Some(ExecuteTransactionOptions {
                    wait: ExecuteTransactionWait::Receipt,
                    ..Default::default()
                }),
            )
            .await
    }))
    .await;

    // Check that we stay in the commit phase, and that the block range and round advances.
    let start = Instant::now();
    let mut round = 0;
    let mut block_range = (0, 0);
    while start.elapsed() < Duration::from_secs(2) {
        for node in &nodes {
            let current_phase = node
                .app
                .sync_query()
                .get_committee_selection_beacon_phase()
                .unwrap();
            let current_round = node
                .app
                .sync_query()
                .get_committee_selection_beacon_round()
                .unwrap();

            // Check that we're in a commit phase.
            assert!(matches!(
                current_phase,
                CommitteeSelectionBeaconPhase::Commit(_)
            ));

            // Check that the block range advances.
            match current_phase {
                CommitteeSelectionBeaconPhase::Commit((start, end)) => {
                    assert!(
                        end - start == genesis.committee_selection_beacon_commit_phase_duration
                    );
                    if block_range != (start, end) {
                        assert!(start > block_range.0 && end > block_range.1);
                        block_range = (start, end);
                    }
                },
                _ => unreachable!(),
            }

            // Check that the round advances.
            if round == 0 {
                round = current_round;
            } else if current_round != round {
                assert_eq!(current_round, round + 1);
                round += 1;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(round > 1);
    assert!(
        block_range.1 > 0
            && block_range.1 > genesis.committee_selection_beacon_reveal_phase_duration
    );

    // Check that the epoch has not changed.
    for node in &nodes {
        assert_eq!(node.get_epoch(), epoch);
    }

    // Shutdown the nodes.
    join_all(nodes.iter_mut().map(|node| node.shutdown())).await;
    join_all(mocked_nodes.iter_mut().map(|node| node.shutdown())).await;
}

#[tokio::test]
async fn test_insufficient_participation_in_reveal_phase() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_node_attempts_reveal_without_committment() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_invalid_reveal_mismatch_with_commit() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_node_submits_commit_outside_of_commit_phase() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_node_submits_reveal_outside_of_reveal_phase() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_node_reuses_old_commitment() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_node_reuses_old_reveal() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_non_committee_node_participation() {
    // TODO(snormore): Implement this test.

    // TODO(snormore): Check that the next commmittee was selected.
}

#[tokio::test]
async fn test_malformed_commit() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_malformed_reveal() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_non_revealing_node_partially_slashed() {
    // TODO(snormore): Implement this test.

    // Check that the node was slashed.

    // Check that if the node attempts to commit in the next round, it will be rejected.

    // Check that the node is not included in sufficient participation for the next round.
}

#[tokio::test]
async fn test_non_revealing_node_fully_slashed() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_high_volume_participation() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_network_delays() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_node_attempts_to_submit_reveal_during_commit_phase() {
    // TODO(snormore): Implement this test.
}

#[tokio::test]
async fn test_multiple_non_revealing_nodes() {
    // TODO(snormore): Implement this test.
}

partial_node_components!(TestNodeComponents {
    ConfigProviderInterface = JsonConfigProvider;
    ApplicationInterface = Application<Self>;
    CommitteeBeaconInterface = CommitteeBeaconComponent<Self>;
    NotifierInterface = Notifier<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    ConsensusInterface = MockConsensus<Self>;
    ForwarderInterface = MockForwarder<Self>;
    SignerInterface = Signer<Self>;
});

partial_node_components!(TestNodeComponentsWithMockCommitteeBeacon {
    ConfigProviderInterface = JsonConfigProvider;
    ApplicationInterface = Application<Self>;
    // CommitteeBeaconInterface = MockCommitteeBeaconComponent<Self>;
    NotifierInterface = Notifier<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    ConsensusInterface = MockConsensus<Self>;
    ForwarderInterface = MockForwarder<Self>;
    SignerInterface = Signer<Self>;
});

struct LocalTestNode<C: NodeComponents> {
    inner: Node<C>,
    _temp_dir: TempDir,

    app: fdi::Ref<C::ApplicationInterface>,
    keystore: fdi::Ref<C::KeystoreInterface>,
    notifier: fdi::Ref<C::NotifierInterface>,
    forwarder: fdi::Ref<C::ForwarderInterface>,
}

impl<C: NodeComponents> LocalTestNode<C> {
    pub async fn start(&mut self) {
        self.inner.start().await;
    }

    pub async fn shutdown(&mut self) {
        self.inner.shutdown().await;
    }

    pub async fn node_transaction_client(&self) -> TransactionClient<C> {
        TransactionClient::<C>::new(
            self.app.sync_query(),
            self.notifier.clone(),
            self.forwarder.mempool_socket(),
            TransactionSigner::NodeMain(self.keystore.get_ed25519_sk()),
        )
        .await
    }

    pub fn get_epoch(&self) -> Epoch {
        self.app.sync_query().get_current_epoch()
    }
}

type GenesisMutator = Box<dyn FnOnce(&mut Genesis)>;

struct TestNodeBuilder {
    genesis: Option<Genesis>,
    genesis_mutator: Option<GenesisMutator>,
    mock_consensus_config: Option<MockConsensusConfig>,
    mock_consensus_group: Option<MockConsensusGroup>,
}

impl TestNodeBuilder {
    pub fn new() -> Self {
        Self {
            genesis: None,
            genesis_mutator: None,
            mock_consensus_config: None,
            mock_consensus_group: None,
        }
    }

    pub fn with_consensus_group(mut self, consensus_group: MockConsensusGroup) -> Self {
        self.mock_consensus_group = Some(consensus_group);
        self
    }

    pub async fn build<C: NodeComponents>(self) -> Result<LocalTestNode<C>> {
        let _ = try_init_tracing();
        let keystore = EphemeralKeystore::<C>::default();
        let temp_dir = tempdir().unwrap();
        let mut genesis = self.genesis.clone();
        if let Some(mutator) = self.genesis_mutator {
            if let Some(genesis) = &mut genesis {
                mutator(genesis);
            }
        }
        let genesis_path = genesis.map(|genesis| {
            genesis
                .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
                .unwrap()
        });
        let mut provider = fdi::Provider::default().with(keystore).with(
            JsonConfigProvider::default()
                .with::<Application<C>>(ApplicationConfig {
                    network: None,
                    genesis_path,
                    storage: StorageConfig::InMemory,
                    db_path: None,
                    db_options: None,
                    dev: None,
                })
                .with::<CommitteeBeaconComponent<C>>(CommitteeBeaconConfig {
                    database: CommitteeBeaconDatabaseConfig {
                        path: temp_dir.path().join("committee-beacon").try_into().unwrap(),
                    },
                    timer: CommitteeBeaconTimerConfig {
                        tick_delay: Duration::from_millis(100),
                    },
                })
                .with::<MockConsensus<C>>(self.mock_consensus_config.unwrap_or(
                    MockConsensusConfig {
                        min_ordering_time: 0,
                        max_ordering_time: 0,
                        probability_txn_lost: 0.0,
                        transactions_to_lose: HashSet::new(),
                        new_block_interval: Duration::from_secs(0),
                    },
                )),
        );
        if let Some(mock_consensus_group) = self.mock_consensus_group {
            provider = provider.with(mock_consensus_group);
        }
        let node = Node::<C>::init_with_provider(provider).map_err(anyhow::Error::from)?;
        Ok(LocalTestNode {
            keystore: node.provider.get::<C::KeystoreInterface>(),
            app: node.provider.get::<C::ApplicationInterface>(),
            notifier: node.provider.get::<C::NotifierInterface>(),
            forwarder: node.provider.get::<C::ForwarderInterface>(),

            inner: node,
            _temp_dir: temp_dir,
        })
    }
}
