use std::time::{Duration, SystemTime};

use fleek_crypto::{AccountOwnerSecretKey, ConsensusSecretKey, NodeSecretKey, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::ApplicationConfig;
use lightning_application::state::QueryRunner;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{Genesis, GenesisNode, NodePorts};
use lightning_interfaces::Weight;
use lightning_node::Node;
use lightning_notifier::Notifier;
use lightning_signer::Signer;
use lightning_test_utils::consensus::{MockConsensus, MockConsensusConfig, MockForwarder};
use lightning_test_utils::e2e::{
    DowncastToTestFullNode,
    TestFullNodeComponentsWithMockConsensus,
    TestNetwork,
};
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_utils::poll::{poll_until, PollUntilError};
use tempfile::tempdir;

use crate::aggregator::ReputationAggregator;
use crate::config::Config;
use crate::measurement_manager::Interactions;
use crate::{MyReputationQuery, MyReputationReporter};

partial_node_components!(TestBinding {
    ConfigProviderInterface = JsonConfigProvider;
    ReputationAggregatorInterface = ReputationAggregator<Self>;
    ApplicationInterface = Application<Self>;
    NotifierInterface = Notifier<Self>;
    ForwarderInterface = MockForwarder<Self>;
    ConsensusInterface = MockConsensus<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    SignerInterface = Signer<Self>;
});

#[tokio::test]
async fn test_query() {
    let keystore = EphemeralKeystore::<TestBinding>::default();
    let (consensus_secret_key, node_secret_key) =
        (keystore.get_bls_sk(), keystore.get_ed25519_sk());
    let (consensus_public_key, node_public_key) =
        (consensus_secret_key.to_pk(), node_secret_key.to_pk());
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();

    let mut genesis = Genesis::default();

    genesis.node_info.push(GenesisNode::new(
        owner_public_key.into(),
        node_public_key,
        "127.0.0.1".parse().unwrap(),
        consensus_public_key,
        "127.0.0.1".parse().unwrap(),
        node_public_key,
        NodePorts {
            primary: 48000_u16,
            worker: 48101_u16,
            mempool: 48202_u16,
            rpc: 48300_u16,
            pool: 48400_u16,
            pinger: 48600_u16,
            handshake: Default::default(),
        },
        None,
        true,
    ));

    let temp_dir = tempdir().unwrap();
    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let mut node = Node::<TestBinding>::init_with_provider(
        fdi::Provider::default()
            .with(
                JsonConfigProvider::default()
                    .with::<Application<TestBinding>>(ApplicationConfig::test(genesis_path))
                    .with::<MockConsensus<TestBinding>>(MockConsensusConfig::default())
                    .with::<ReputationAggregator<TestBinding>>(Config {
                        reporter_buffer_size: 1,
                    }),
            )
            .with(keystore),
    )
    .expect("failed to initialize node");
    node.start().await;

    let rep_reporter = node.provider.get::<MyReputationReporter>();
    let rep_query = node.provider.get::<MyReputationQuery>();

    // Report some measurements for alice and bob.
    let alice = 1;
    let bob = 2;
    rep_reporter.report_sat(alice, Weight::Strong);
    rep_reporter.report_sat(alice, Weight::VeryStrong);
    rep_reporter.report_sat(bob, Weight::Weak);
    rep_reporter.report_unsat(bob, Weight::Strong);

    rep_reporter.report_ping(alice, Some(Duration::from_millis(100)));
    rep_reporter.report_ping(alice, Some(Duration::from_millis(120)));
    rep_reporter.report_ping(bob, Some(Duration::from_millis(300)));
    rep_reporter.report_ping(bob, Some(Duration::from_millis(350)));

    rep_reporter.report_bytes_sent(bob, 1000, None);

    // Check that there are local reputation score for alice and bob.
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    loop {
        tokio::select! {
            //_ = &mut aggregator_handle => {}
            _ = interval.tick() => {
                let alice_rep = rep_query.get_reputation_of(&alice);
                let bob_rep = rep_query.get_reputation_of(&bob);
                if alice_rep.is_some() && bob_rep.is_some() {
                    // We reported better measurements for alice, therefore alice should have a
                    // better local reputation score.
                    assert!(alice_rep.unwrap() > bob_rep.unwrap());
                    break;
                }
            }
        }
    }

    node.shutdown().await;
}

#[tokio::test]
async fn test_submit_measurements() {
    let keystore = EphemeralKeystore::<TestBinding>::default();
    let (consensus_secret_key, node_secret_key) =
        (keystore.get_bls_sk(), keystore.get_ed25519_sk());
    let (consensus_public_key, node_public_key) =
        (consensus_secret_key.to_pk(), node_secret_key.to_pk());
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();

    let peer_owner_public_key = AccountOwnerSecretKey::generate();
    let peer_secret_key = NodeSecretKey::generate();
    let peer_public_key = peer_secret_key.to_pk();
    let peer_consensus_secret_key = ConsensusSecretKey::generate();
    let peer_consensus_public_key = peer_consensus_secret_key.to_pk();

    let mut genesis = Genesis::default();

    genesis.node_info.push(GenesisNode::new(
        owner_public_key.into(),
        node_public_key,
        "127.0.0.1".parse().unwrap(),
        consensus_public_key,
        "127.0.0.1".parse().unwrap(),
        node_public_key,
        NodePorts {
            primary: 48000,
            worker: 48101,
            mempool: 48102,
            rpc: 48103,
            pool: 48104,
            pinger: 38106,
            handshake: Default::default(),
        },
        None,
        true,
    ));

    genesis.node_info.push(GenesisNode::new(
        peer_owner_public_key.to_pk().into(),
        peer_public_key,
        "127.0.0.1".parse().unwrap(),
        peer_consensus_public_key,
        "127.0.0.1".parse().unwrap(),
        peer_public_key,
        NodePorts {
            primary: 38000,
            worker: 38101,
            mempool: 38102,
            rpc: 38103,
            pool: 38104,
            pinger: 38106,
            handshake: Default::default(),
        },
        None,
        true,
    ));

    let epoch_start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    genesis.epoch_start = epoch_start;
    genesis.epoch_time = 4000; // millis

    let temp_dir = tempdir().unwrap();
    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let mut node = Node::<TestBinding>::init_with_provider(
        fdi::Provider::default()
            .with(
                JsonConfigProvider::default()
                    .with::<Application<TestBinding>>(ApplicationConfig::test(genesis_path))
                    .with::<MockConsensus<TestBinding>>(MockConsensusConfig::default())
                    .with::<ReputationAggregator<TestBinding>>(Config {
                        reporter_buffer_size: 1,
                    }),
            )
            .with(keystore),
    )
    .expect("failed to initialize node");
    node.start().await;

    let query_runner: fdi::Ref<QueryRunner> = node.provider.get();
    let rep_reporter = node.provider.get::<MyReputationReporter>();

    // Report some measurements to the reputation aggregator.
    let peer_index = query_runner.pubkey_to_index(&peer_public_key).unwrap();
    rep_reporter.report_sat(peer_index, Weight::Weak);
    rep_reporter.report_sat(peer_index, Weight::Strong);
    rep_reporter.report_ping(peer_index, Some(Duration::from_millis(300)));
    rep_reporter.report_ping(peer_index, Some(Duration::from_millis(100)));
    rep_reporter.report_bytes_sent(peer_index, 10_000, Some(Duration::from_millis(100)));
    rep_reporter.report_bytes_received(peer_index, 20_000, Some(Duration::from_millis(100)));
    rep_reporter.report_hops(peer_index, 4);

    let reporting_node_index = query_runner.pubkey_to_index(&node_public_key).unwrap();

    let mut interval = tokio::time::interval(Duration::from_millis(100));
    loop {
        tokio::select! {
            _ = interval.tick() => {
                let measurements =
                    query_runner.get_reputation_measurements(&peer_index).unwrap_or(Vec::new());
                if !measurements.is_empty() {
                    // Make sure that the reported measurements were submitted to the application
                    // state.
                    assert_eq!(measurements.len(), 1);
                    assert_eq!(measurements[0].reporting_node, reporting_node_index);
                    assert_eq!(
                        measurements[0].measurements.latency,
                        Some(Duration::from_millis(200))
                    );
                    let interactions = Interactions::get_weight(Weight::Weak)
                        + Interactions::get_weight(Weight::Strong);
                    assert_eq!(measurements[0].measurements.interactions, Some(interactions));
                    assert_eq!(measurements[0].measurements.bytes_received, Some(20_000));
                    assert_eq!(measurements[0].measurements.bytes_sent, Some(10_000));
                    assert_eq!(measurements[0].measurements.inbound_bandwidth, Some(100));
                    assert_eq!(measurements[0].measurements.outbound_bandwidth, Some(200));
                    assert_eq!(measurements[0].measurements.hops, Some(4));
                    break;
                }
            }
        }
    }

    node.shutdown().await;
}

#[tokio::test]
async fn test_reputation_calculation_and_query() {
    let mut network = TestNetwork::builder()
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(4)
        .await
        .with_genesis_mutator(|genesis| {
            genesis.epoch_start = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            genesis.epoch_time = 4000;
        })
        .build()
        .await
        .unwrap();
    let node1 = network
        .node(0)
        .downcast::<TestFullNodeComponentsWithMockConsensus>();
    let node2 = network
        .node(1)
        .downcast::<TestFullNodeComponentsWithMockConsensus>();
    let app_query = node1.app_query().clone();

    let reporter1 = node1.reputation_reporter().clone();
    let reporter2 = node2.reputation_reporter().clone();

    let alice = network.node(2).index();
    let bob = network.node(3).index();

    // Report some measurements for alice (node 2) and bob (node 3), from node 0 and node 1.
    reporter1.report_sat(alice, Weight::Strong);
    reporter1.report_ping(alice, Some(Duration::from_millis(100)));
    reporter1.report_bytes_sent(alice, 10_000, Some(Duration::from_millis(100)));
    reporter1.report_bytes_received(alice, 20_000, Some(Duration::from_millis(100)));

    reporter1.report_sat(bob, Weight::Weak);
    reporter1.report_ping(bob, Some(Duration::from_millis(300)));
    reporter1.report_bytes_sent(bob, 12_000, Some(Duration::from_millis(300)));
    reporter1.report_bytes_received(bob, 23_000, Some(Duration::from_millis(400)));

    reporter2.report_sat(alice, Weight::Strong);
    reporter2.report_ping(alice, Some(Duration::from_millis(120)));
    reporter2.report_bytes_sent(alice, 11_000, Some(Duration::from_millis(90)));
    reporter2.report_bytes_received(alice, 22_000, Some(Duration::from_millis(110)));

    reporter2.report_sat(bob, Weight::Weak);
    reporter2.report_ping(bob, Some(Duration::from_millis(250)));
    reporter2.report_bytes_sent(bob, 9_000, Some(Duration::from_millis(280)));
    reporter2.report_bytes_received(bob, 19_000, Some(Duration::from_millis(350)));

    // Wait until all measurements have been submitted to the application.
    poll_until(
        || async {
            let alice_rep_count = app_query
                .get_reputation_measurements(&alice)
                .unwrap_or(Vec::new())
                .iter()
                .filter(|m| m.measurements.interactions.is_some())
                .count();
            let bob_rep_count = app_query
                .get_reputation_measurements(&bob)
                .unwrap_or(Vec::new())
                .iter()
                .filter(|m| m.measurements.interactions.is_some())
                .count();

            (alice_rep_count == 2 && bob_rep_count == 2)
                .then_some(())
                .ok_or(PollUntilError::ConditionNotSatisfied)
        },
        Duration::from_secs(20),
        Duration::from_millis(100),
    )
    .await
    .unwrap();

    // Change epoch and wait for it to be complete.
    network.change_epoch_and_wait_for_complete().await.unwrap();

    // Check that the reputation scores have been calculated correctly, and that node1 has a higher
    // reputation score than node2, because reported measurements were better.
    let node1_rep = app_query.get_reputation_score(&alice).unwrap();
    let node2_rep = app_query.get_reputation_score(&bob).unwrap();
    assert!(node1_rep > node2_rep);

    // Shutdown the network.
    network.shutdown().await;
}
