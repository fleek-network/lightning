use std::collections::HashSet;
use std::time::{Duration, SystemTime};

use fleek_crypto::{AccountOwnerSecretKey, ConsensusSecretKey, NodeSecretKey, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::ApplicationConfig;
use lightning_application::state::QueryRunner;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    Genesis,
    GenesisNode,
    NodePorts,
    UpdateMethod,
    UpdatePayload,
    UpdateRequest,
};
use lightning_interfaces::Weight;
use lightning_notifier::Notifier;
use lightning_signer::Signer;
use lightning_test_utils::consensus::{
    Config as ConsensusConfig,
    MockConsensus,
    MockConsensusGroup,
    MockForwarder,
};
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_utils::application::QueryRunnerExt;
use tempfile::tempdir;

use crate::aggregator::ReputationAggregator;
use crate::config::Config;
use crate::measurement_manager::Interactions;
use crate::{MyReputationQuery, MyReputationReporter};

partial!(TestBinding {
    ConfigProviderInterface = JsonConfigProvider;
    ReputationAggregatorInterface = ReputationAggregator<Self>;
    ApplicationInterface = Application<Self>;
    NotifierInterface = Notifier<Self>;
    ForwarderInterface = MockForwarder<Self>;
    ConsensusInterface = MockConsensus<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    SignerInterface = Signer<Self>;
});

fn get_genesis_committee(
    num_members: usize,
) -> (Vec<GenesisNode>, Vec<EphemeralKeystore<TestBinding>>) {
    let mut keystores = Vec::new();
    let mut committee = Vec::new();
    (0..num_members).for_each(|i| {
        let keystore = EphemeralKeystore::default();
        let (consensus_secret_key, node_secret_key) =
            (keystore.get_bls_sk(), keystore.get_ed25519_sk());
        let (consensus_public_key, node_public_key) =
            (consensus_secret_key.to_pk(), node_secret_key.to_pk());
        let owner_secret_key = AccountOwnerSecretKey::generate();
        let owner_public_key = owner_secret_key.to_pk();

        committee.push(GenesisNode::new(
            owner_public_key.into(),
            node_public_key,
            "127.0.0.1".parse().unwrap(),
            consensus_public_key,
            "127.0.0.1".parse().unwrap(),
            node_public_key,
            NodePorts {
                primary: 8000 + i as u16,
                worker: 8100 + i as u16,
                mempool: 8200 + i as u16,
                rpc: 8300 + i as u16,
                pool: 8400 + i as u16,
                pinger: 8600 + i as u16,
                handshake: Default::default(),
            },
            None,
            true,
        ));
        keystores.push(keystore);
    });
    (committee, keystores)
}

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
                    .with::<MockConsensus<TestBinding>>(ConsensusConfig {
                        min_ordering_time: 0,
                        max_ordering_time: 1,
                        probability_txn_lost: 0.0,
                        transactions_to_lose: HashSet::new(),
                        new_block_interval: Duration::from_secs(5),
                    })
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
                    .with::<MockConsensus<TestBinding>>(ConsensusConfig {
                        min_ordering_time: 0,
                        max_ordering_time: 1,
                        probability_txn_lost: 0.0,
                        transactions_to_lose: HashSet::new(),
                        new_block_interval: Duration::from_secs(5),
                    })
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
    // The application will only calculate reputation scores for nodes if multiple nodes
    // have reported measurements. Therefore, we need to create two reputation aggregators, two
    // signers, and two consensus services that will receive a socket for the same application
    // service.
    let keystore1 = EphemeralKeystore::<TestBinding>::default();
    let keystore2 = EphemeralKeystore::<TestBinding>::default();
    let (consensus_secret_key1, node_secret_key1) =
        (keystore1.get_bls_sk(), keystore1.get_ed25519_sk());
    let (consensus_secret_key2, node_secret_key2) =
        (keystore2.get_bls_sk(), keystore2.get_ed25519_sk());

    let (committee, mut keystores) = get_genesis_committee(4);
    let mut genesis = Genesis::default();
    let chain_id = genesis.chain_id;

    genesis.node_info = committee;

    let node_public_key1 = node_secret_key1.to_pk();
    let consensus_public_key1 = consensus_secret_key1.to_pk();
    let owner_secret_key1 = AccountOwnerSecretKey::generate();
    let owner_public_key1 = owner_secret_key1.to_pk();

    let consensus_group = MockConsensusGroup::new(
        ConsensusConfig {
            min_ordering_time: 0,
            max_ordering_time: 1,
            probability_txn_lost: 0.0,
            transactions_to_lose: HashSet::new(),
            new_block_interval: Duration::from_secs(5),
        },
        None,
    );

    genesis.node_info.push(GenesisNode::new(
        owner_public_key1.into(),
        node_public_key1,
        "127.0.0.1".parse().unwrap(),
        consensus_public_key1,
        "127.0.0.1".parse().unwrap(),
        node_public_key1,
        NodePorts {
            primary: 48000,
            worker: 48101,
            mempool: 48102,
            rpc: 48103,
            pool: 48104,
            pinger: 48106,
            handshake: Default::default(),
        },
        None,
        true,
    ));
    keystores.push(keystore1.clone());

    let node_public_key2 = node_secret_key2.to_pk();
    let consensus_public_key2 = consensus_secret_key2.to_pk();
    let owner_secret_key2 = AccountOwnerSecretKey::generate();
    let owner_public_key2 = owner_secret_key2.to_pk();

    genesis.node_info.push(GenesisNode::new(
        owner_public_key2.into(),
        node_public_key2,
        "127.0.0.1".parse().unwrap(),
        consensus_public_key2,
        "127.0.0.1".parse().unwrap(),
        node_public_key2,
        NodePorts {
            primary: 48000,
            worker: 48101,
            mempool: 48102,
            rpc: 48103,
            pool: 48104,
            pinger: 48106,
            handshake: Default::default(),
        },
        None,
        true,
    ));
    keystores.push(keystore2.clone());

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

    let mut node1 = Node::<TestBinding>::init_with_provider(
        fdi::Provider::default()
            .with(
                JsonConfigProvider::default()
                    .with::<Application<TestBinding>>(ApplicationConfig::test(genesis_path.clone()))
                    .with::<ReputationAggregator<TestBinding>>(Config {
                        reporter_buffer_size: 1,
                    }),
            )
            .with(consensus_group.clone())
            .with(keystore1),
    )
    .expect("failed to initialize node");
    node1.start().await;

    let rep_reporter1 = node1.provider.get::<MyReputationReporter>();
    let forwarder_socket = node1
        .provider
        .get::<MockForwarder<TestBinding>>()
        .mempool_socket();

    let mut node2 = Node::<TestBinding>::init_with_provider(
        fdi::Provider::default()
            .with(
                JsonConfigProvider::default()
                    .with::<Application<TestBinding>>(ApplicationConfig::test(genesis_path))
                    .with::<ReputationAggregator<TestBinding>>(Config {
                        reporter_buffer_size: 1,
                    }),
            )
            .with(consensus_group)
            .with(keystore2),
    )
    .expect("failed to initialize node");
    node2.start().await;
    let query_runner: fdi::Ref<QueryRunner> = node2.provider.get();
    let rep_reporter2 = node2.provider.get::<MyReputationReporter>();

    // Both nodes report measurements for two peers (alice and bob).
    // note(dalton): Refactored to not include measurements for non white listed nodes so have to
    // switch Alice and bob to the keys we added to the committee
    let alice = query_runner
        .pubkey_to_index(&keystores[0].get_ed25519_pk())
        .unwrap();
    let bob = query_runner
        .pubkey_to_index(&keystores[1].get_ed25519_pk())
        .unwrap();

    rep_reporter1.report_sat(alice, Weight::Strong);
    rep_reporter1.report_ping(alice, Some(Duration::from_millis(100)));
    rep_reporter1.report_bytes_sent(alice, 10_000, Some(Duration::from_millis(100)));
    rep_reporter1.report_bytes_received(alice, 20_000, Some(Duration::from_millis(100)));

    rep_reporter1.report_sat(bob, Weight::Weak);
    rep_reporter1.report_ping(bob, Some(Duration::from_millis(300)));
    rep_reporter1.report_bytes_sent(bob, 12_000, Some(Duration::from_millis(300)));
    rep_reporter1.report_bytes_received(bob, 23_000, Some(Duration::from_millis(400)));

    rep_reporter2.report_sat(alice, Weight::Strong);
    rep_reporter2.report_ping(alice, Some(Duration::from_millis(120)));
    rep_reporter2.report_bytes_sent(alice, 11_000, Some(Duration::from_millis(90)));
    rep_reporter2.report_bytes_received(alice, 22_000, Some(Duration::from_millis(110)));

    rep_reporter2.report_sat(bob, Weight::Weak);
    rep_reporter2.report_ping(bob, Some(Duration::from_millis(250)));
    rep_reporter2.report_bytes_sent(bob, 9_000, Some(Duration::from_millis(280)));
    rep_reporter2.report_bytes_received(bob, 19_000, Some(Duration::from_millis(350)));

    let mut interval = tokio::time::interval(Duration::from_millis(100));
    loop {
        interval.tick().await;
        let measure_alice = query_runner
            .get_reputation_measurements(&alice)
            .unwrap_or(Vec::new());
        let measure_bob = query_runner
            .get_reputation_measurements(&bob)
            .unwrap_or(Vec::new());
        if measure_alice.len() == 2 && measure_bob.len() == 2 {
            // Wait until all measurements were submitted to the application.
            break;
        }
    }

    // Change epoch to trigger reputation score calculation.
    let required_signals = 2 * keystores.len() / 3 + 1;
    for node in keystores.iter().take(required_signals) {
        let sk = node.get_ed25519_sk();
        let method = UpdateMethod::ChangeEpoch { epoch: 0 };
        // If the committee member is either one of the nodes from this test, we have to increment
        // the nonce, since the nodes already send a transaction containing the measurements.
        let nonce = if sk == node_secret_key1 || sk == node_secret_key2 {
            2
        } else {
            1
        };
        let payload = UpdatePayload {
            sender: node.get_ed25519_pk().into(),
            nonce,
            method,
            chain_id,
        };
        let digest = payload.to_digest();
        let signature = sk.sign(&digest);
        let req = UpdateRequest {
            signature: signature.into(),
            payload,
        };

        forwarder_socket.run(req.into()).await.unwrap();
    }
    // Make sure that the epoch change happened.
    assert_eq!(query_runner.get_epoch_info().epoch, 1);

    // After a long journey, here comes the actual testing.
    // Test that reputation scores were calculated for both alice and bob.
    // Also test that alice received a higher reputation, because the reported measurements were
    // better.
    let alice_rep = query_runner
        .get_reputation_score(&alice)
        .expect("Reputation score for alice is missing");
    let bob_rep = query_runner
        .get_reputation_score(&bob)
        .expect("Reputation score for bob is missing");
    assert!(alice_rep > bob_rep);

    node1.shutdown().await;
    node2.shutdown().await;
}
