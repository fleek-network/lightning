use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, SystemTime},
};

use draco_application::{
    app::Application,
    config::{Config as AppConfig, Mode},
    genesis::{Genesis, GenesisCommittee},
};
use draco_interfaces::{
    application::ApplicationInterface,
    common::WithStartAndShutdown,
    consensus::ConsensusInterface,
    notifier::NotifierInterface,
    reputation::{ReputationAggregatorInterface, ReputationReporterInterface},
    signer::SignerInterface,
    types::{Block, UpdateMethod, UpdatePayload, UpdateRequest},
    ReputationQueryInteface, SyncQueryRunnerInterface, ToDigest, Weight,
};
use draco_notifier::Notifier;
use draco_signer::{Config as SignerConfig, Signer};
use draco_test_utils::{
    consensus::{Config as ConsensusConfig, MockConsensus},
    empty_interfaces::MockGossip,
};
use fleek_crypto::{
    AccountOwnerSecretKey, NodeNetworkingSecretKey, NodePublicKey, NodeSecretKey, PublicKey,
    SecretKey,
};

use crate::{aggregator::ReputationAggregator, config::Config, measurement_manager::Interactions};

// TODO(matthias): the same struct and `get_genesis_committee` already exist in the application
// tests. This should be moved to test-utils, however, fot his to work, we have to move the genesis
// structs to the types interface.
struct GenesisCommitteeKeystore {
    _owner_secret_key: AccountOwnerSecretKey,
    node_secret_key: NodeSecretKey,
    _network_secret_key: NodeNetworkingSecretKey,
    _worker_secret_key: NodeNetworkingSecretKey,
}

fn get_genesis_committee(
    num_members: usize,
) -> (Vec<GenesisCommittee>, Vec<GenesisCommitteeKeystore>) {
    let mut keystore = Vec::new();
    let mut committee = Vec::new();
    (0..num_members).for_each(|i| {
        let node_secret_key = NodeSecretKey::generate();
        let node_public_key = node_secret_key.to_pk();
        let network_secret_key = NodeNetworkingSecretKey::generate();
        let network_public_key = network_secret_key.to_pk();
        let owner_secret_key = AccountOwnerSecretKey::generate();
        let owner_public_key = owner_secret_key.to_pk();
        committee.push(GenesisCommittee::new(
            owner_public_key.to_base64(),
            node_public_key.to_base64(),
            format!("/ip4/127.0.0.1/udp/800{i}"),
            network_public_key.to_base64(),
            format!("/ip4/127.0.0.1/udp/810{i}/http"),
            network_public_key.to_base64(),
            format!("/ip4/127.0.0.1/tcp/810{i}/http"),
            None,
        ));
        keystore.push(GenesisCommitteeKeystore {
            _owner_secret_key: owner_secret_key,
            node_secret_key,
            _network_secret_key: network_secret_key,
            _worker_secret_key: network_secret_key,
        });
    });
    (committee, keystore)
}

#[tokio::test]
async fn test_query() {
    let signer_config = SignerConfig::default();
    let mut signer = Signer::init(signer_config).await.unwrap();

    let mut genesis = Genesis::load().unwrap();
    let (network_secret_key, secret_key) = signer.get_sk();
    let public_key = secret_key.to_pk();
    let network_public_key = network_secret_key.to_pk();
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();

    genesis.committee.push(GenesisCommittee::new(
        owner_public_key.to_base64(),
        public_key.to_base64(),
        "/ip4/127.0.0.1/udp/48000".to_owned(),
        network_public_key.to_base64(),
        "/ip4/127.0.0.1/udp/48101/http".to_owned(),
        network_public_key.to_base64(),
        "/ip4/127.0.0.1/tcp/48102/http".to_owned(),
        None,
    ));

    let app = Application::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .await
    .unwrap();
    app.start().await;

    let (update_socket, query_runner) = (app.transaction_executor(), app.sync_query());

    let consensus_config = ConsensusConfig {
        min_ordering_time: 0,
        max_ordering_time: 1,
        probability_txn_lost: 0.0,
        transactions_to_lose: HashSet::new(),
        new_block_interval: Duration::from_secs(5),
    };
    let consensus = MockConsensus::init(
        consensus_config,
        &signer,
        update_socket.clone(),
        query_runner.clone(),
        Arc::new(MockGossip {}),
    )
    .await
    .unwrap();

    signer.provide_mempool(consensus.mempool());
    signer.provide_query_runner(query_runner.clone());
    signer.provide_new_block_notify(consensus.new_block_notifier());
    signer.start().await;
    consensus.start().await;

    let notifier = Notifier::init(query_runner.clone());
    let config = Config {
        reporter_buffer_size: 1,
    };
    let rep_aggregator = ReputationAggregator::init(config, signer.get_socket(), notifier)
        .await
        .unwrap();

    let rep_reporter = rep_aggregator.get_reporter();
    let rep_query = rep_aggregator.get_query();
    let mut aggregator_handle = tokio::spawn(async move {
        rep_aggregator.start().await.unwrap();
    });
    // Report some measurements for alice and bob.
    let alice = NodePublicKey([1; 96]);
    let bob = NodePublicKey([2; 96]);
    rep_reporter.report_sat(&alice, Weight::Strong);
    rep_reporter.report_sat(&alice, Weight::VeryStrong);
    rep_reporter.report_sat(&bob, Weight::Weak);
    rep_reporter.report_unsat(&bob, Weight::Strong);

    rep_reporter.report_latency(&alice, Duration::from_millis(100));
    rep_reporter.report_latency(&alice, Duration::from_millis(120));
    rep_reporter.report_latency(&bob, Duration::from_millis(300));
    rep_reporter.report_latency(&bob, Duration::from_millis(350));

    rep_reporter.report_bytes_sent(&bob, 1000, None);

    // Check that there are local reputation score for alice and bob.
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    loop {
        tokio::select! {
            _ = &mut aggregator_handle => {}
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
}

#[tokio::test]
async fn test_submit_measurements() {
    let signer_config = SignerConfig::default();
    let mut signer = Signer::init(signer_config).await.unwrap();

    let mut genesis = Genesis::load().unwrap();
    let (network_secret_key, secret_key) = signer.get_sk();
    let public_key = secret_key.to_pk();
    let network_public_key = network_secret_key.to_pk();
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();

    genesis.committee.push(GenesisCommittee::new(
        owner_public_key.to_base64(),
        public_key.to_base64(),
        "/ip4/127.0.0.1/udp/48000".to_owned(),
        network_public_key.to_base64(),
        "/ip4/127.0.0.1/udp/48101/http".to_owned(),
        network_public_key.to_base64(),
        "/ip4/127.0.0.1/tcp/48102/http".to_owned(),
        None,
    ));

    let epoch_start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    genesis.epoch_start = epoch_start;
    genesis.epoch_time = 4000; // millis

    let app = Application::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .await
    .unwrap();
    app.start().await;

    let (update_socket, query_runner) = (app.transaction_executor(), app.sync_query());

    let consensus_config = ConsensusConfig {
        min_ordering_time: 0,
        max_ordering_time: 1,
        probability_txn_lost: 0.0,
        transactions_to_lose: HashSet::new(),
        new_block_interval: Duration::from_secs(5),
    };
    let consensus = MockConsensus::init(
        consensus_config,
        &signer,
        update_socket.clone(),
        query_runner.clone(),
        Arc::new(MockGossip {}),
    )
    .await
    .unwrap();

    signer.provide_mempool(consensus.mempool());
    signer.provide_query_runner(query_runner.clone());
    signer.provide_new_block_notify(consensus.new_block_notifier());
    signer.start().await;
    consensus.start().await;

    let notifier = Notifier::init(query_runner.clone());
    let config = Config {
        reporter_buffer_size: 1,
    };
    let rep_aggregator = ReputationAggregator::init(config, signer.get_socket(), notifier)
        .await
        .unwrap();

    let rep_reporter = rep_aggregator.get_reporter();
    let mut aggregator_handle = tokio::spawn(async move {
        rep_aggregator.start().await.unwrap();
    });

    // Report some measurements to the reputation aggregator.
    let peer = NodePublicKey([1; 96]);
    rep_reporter.report_sat(&peer, Weight::Weak);
    rep_reporter.report_sat(&peer, Weight::Strong);
    rep_reporter.report_latency(&peer, Duration::from_millis(300));
    rep_reporter.report_latency(&peer, Duration::from_millis(100));
    rep_reporter.report_bytes_sent(&peer, 10_000, Some(Duration::from_millis(100)));
    rep_reporter.report_bytes_received(&peer, 20_000, Some(Duration::from_millis(100)));
    rep_reporter.report_hops(&peer, 4);

    let mut interval = tokio::time::interval(Duration::from_millis(100));
    loop {
        tokio::select! {
            _ = &mut aggregator_handle => {}
            _ = interval.tick() => {
                let measurements = query_runner.get_rep_measurements(peer);
                if !measurements.is_empty() {
                    // Make sure that the reported measurements were submitted to the application
                    // state.
                    assert_eq!(measurements.len(), 1);
                    assert_eq!(measurements[0].reporting_node, public_key);
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
}

#[tokio::test]
async fn test_reputation_calculation_and_query() {
    // The application will only calculate reputation scores for nodes that if multiple nodes
    // have reported measurements. Therefore, we need to create two reputation aggregators, two
    // signers, and two consensus services that will receive a socket for the same application
    // service.
    let mut signer1 = Signer::init(SignerConfig::default()).await.unwrap();
    let mut signer2 = Signer::init(SignerConfig::default()).await.unwrap();

    let (committee, mut keystore) = get_genesis_committee(4);
    let mut genesis = Genesis::load().unwrap();
    genesis.committee = committee;

    let (network_secret_key1, secret_key1) = signer1.get_sk();
    let public_key1 = secret_key1.to_pk();
    let network_public_key1 = network_secret_key1.to_pk();
    let owner_secret_key1 = AccountOwnerSecretKey::generate();
    let owner_public_key1 = owner_secret_key1.to_pk();

    genesis.committee.push(GenesisCommittee::new(
        owner_public_key1.to_base64(),
        public_key1.to_base64(),
        "/ip4/127.0.0.1/udp/48000".to_owned(),
        network_public_key1.to_base64(),
        "/ip4/127.0.0.1/udp/48101/http".to_owned(),
        network_public_key1.to_base64(),
        "/ip4/127.0.0.1/tcp/48102/http".to_owned(),
        None,
    ));
    keystore.push(GenesisCommitteeKeystore {
        _owner_secret_key: owner_secret_key1,
        node_secret_key: secret_key1,
        _network_secret_key: network_secret_key1,
        _worker_secret_key: network_secret_key1,
    });

    let (network_secret_key2, secret_key2) = signer2.get_sk();
    let public_key2 = secret_key2.to_pk();
    let network_public_key2 = network_secret_key2.to_pk();
    let owner_secret_key2 = AccountOwnerSecretKey::generate();
    let owner_public_key2 = owner_secret_key2.to_pk();

    genesis.committee.push(GenesisCommittee::new(
        owner_public_key2.to_base64(),
        public_key2.to_base64(),
        "/ip4/127.0.0.1/udp/48001".to_owned(),
        network_public_key2.to_base64(),
        "/ip4/127.0.0.1/udp/48102/http".to_owned(),
        network_public_key2.to_base64(),
        "/ip4/127.0.0.1/tcp/48103/http".to_owned(),
        None,
    ));
    keystore.push(GenesisCommitteeKeystore {
        _owner_secret_key: owner_secret_key2,
        node_secret_key: secret_key2,
        _network_secret_key: network_secret_key2,
        _worker_secret_key: network_secret_key2,
    });

    let epoch_start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    genesis.epoch_start = epoch_start;
    genesis.epoch_time = 4000; // millis

    let app = Application::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .await
    .unwrap();
    app.start().await;

    let (update_socket, query_runner) = (app.transaction_executor(), app.sync_query());

    let mock_gossip = Arc::new(MockGossip {});
    let consensus_config = ConsensusConfig {
        min_ordering_time: 0,
        max_ordering_time: 1,
        probability_txn_lost: 0.0,
        transactions_to_lose: HashSet::new(),
        new_block_interval: Duration::from_secs(5),
    };
    let consensus1 = MockConsensus::init(
        consensus_config.clone(),
        &signer1,
        update_socket.clone(),
        query_runner.clone(),
        mock_gossip.clone(),
    )
    .await
    .unwrap();
    let consensus2 = MockConsensus::init(
        consensus_config,
        &signer2,
        update_socket.clone(),
        query_runner.clone(),
        mock_gossip.clone(),
    )
    .await
    .unwrap();

    signer1.provide_mempool(consensus1.mempool());
    signer2.provide_mempool(consensus2.mempool());
    signer1.provide_query_runner(query_runner.clone());
    signer2.provide_query_runner(query_runner.clone());
    signer1.provide_new_block_notify(consensus1.new_block_notifier());
    signer2.provide_new_block_notify(consensus2.new_block_notifier());
    signer1.start().await;
    signer2.start().await;
    consensus1.start().await;
    consensus2.start().await;

    let notifier1 = Notifier::init(query_runner.clone());
    let notifier2 = Notifier::init(query_runner.clone());
    let config = Config {
        reporter_buffer_size: 1,
    };
    let rep_aggregator1 =
        ReputationAggregator::init(config.clone(), signer1.get_socket(), notifier1)
            .await
            .unwrap();
    let rep_aggregator2 = ReputationAggregator::init(config, signer2.get_socket(), notifier2)
        .await
        .unwrap();

    let rep_reporter1 = rep_aggregator1.get_reporter();
    let rep_reporter2 = rep_aggregator2.get_reporter();
    let mut aggregator_handle1 = tokio::spawn(async move {
        rep_aggregator1.start().await.unwrap();
    });
    let mut aggregator_handle2 = tokio::spawn(async move {
        rep_aggregator2.start().await.unwrap();
    });

    // Both nodes report measurements for two peers (alice and bob).
    let alice = NodePublicKey([1; 96]);
    let bob = NodePublicKey([2; 96]);
    rep_reporter1.report_sat(&alice, Weight::Strong);
    rep_reporter1.report_latency(&alice, Duration::from_millis(100));
    rep_reporter1.report_bytes_sent(&alice, 10_000, Some(Duration::from_millis(100)));
    rep_reporter1.report_bytes_received(&alice, 20_000, Some(Duration::from_millis(100)));

    rep_reporter1.report_sat(&bob, Weight::Weak);
    rep_reporter1.report_latency(&bob, Duration::from_millis(300));
    rep_reporter1.report_bytes_sent(&bob, 12_000, Some(Duration::from_millis(300)));
    rep_reporter1.report_bytes_received(&bob, 23_000, Some(Duration::from_millis(400)));

    rep_reporter2.report_sat(&alice, Weight::Strong);
    rep_reporter2.report_latency(&alice, Duration::from_millis(120));
    rep_reporter2.report_bytes_sent(&alice, 11_000, Some(Duration::from_millis(90)));
    rep_reporter2.report_bytes_received(&alice, 22_000, Some(Duration::from_millis(110)));

    rep_reporter2.report_sat(&bob, Weight::Weak);
    rep_reporter2.report_latency(&bob, Duration::from_millis(250));
    rep_reporter2.report_bytes_sent(&bob, 9_000, Some(Duration::from_millis(280)));
    rep_reporter2.report_bytes_received(&bob, 19_000, Some(Duration::from_millis(350)));

    let mut interval = tokio::time::interval(Duration::from_millis(100));
    loop {
        tokio::select! {
            _ = &mut aggregator_handle1 => {}
            _ = &mut aggregator_handle2 => {}
            _ = interval.tick() => {
                let measure_alice = query_runner.get_rep_measurements(alice);
                let measure_bob = query_runner.get_rep_measurements(bob);
                if measure_alice.len() == 2 && measure_bob.len() == 2 {
                    // Wait until all measurements were submitted to the application.
                    break;
                }
            }
        }
    }

    // Change epoch to trigger reputation score calculation.
    let required_signals = 2 * keystore.len() / 3 + 1;
    for node in keystore.iter().take(required_signals) {
        let method = UpdateMethod::ChangeEpoch { epoch: 0 };
        // If the committee member is either one of the nodes from this test, we have to increment
        // the nonce, since the nodes already send a transaction containing the measurements.
        let nonce = if node.node_secret_key == secret_key1 || node.node_secret_key == secret_key2 {
            2
        } else {
            1
        };
        let payload = UpdatePayload { nonce, method };
        let digest = payload.to_digest();
        let signature = node.node_secret_key.sign(&digest);
        let req = UpdateRequest {
            sender: node.node_secret_key.to_pk().into(),
            signature: signature.into(),
            payload,
        };
        let _res = update_socket
            .run(Block {
                transactions: vec![req],
            })
            .await
            .map_err(|r| anyhow::anyhow!(format!("{r:?}")))
            .unwrap();
    }
    // Make sure that the epoch change happened.
    assert_eq!(query_runner.get_epoch_info().epoch, 1);

    // After a long journey, here comes the actual testing.
    // Test that reputation scores were calculated for both alice and bob.
    // Also test that alice received a higher reputation, because the reported measurements were
    // better.
    let alice_rep = query_runner
        .get_reputation(&alice)
        .expect("Reputation score for alice is missing");
    let bob_rep = query_runner
        .get_reputation(&bob)
        .expect("Reputation score for bob is missing");
    assert!(alice_rep > bob_rep);
}
