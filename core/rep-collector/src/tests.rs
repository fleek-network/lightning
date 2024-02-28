use std::collections::HashSet;
use std::time::{Duration, SystemTime};

use fleek_crypto::{AccountOwnerSecretKey, ConsensusSecretKey, NodeSecretKey, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::{Config as AppConfig, Mode, StorageConfig};
use lightning_application::genesis::{Genesis, GenesisNode};
use lightning_interfaces::application::ApplicationInterface;
use lightning_interfaces::common::WithStartAndShutdown;
use lightning_interfaces::consensus::ConsensusInterface;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::notifier::NotifierInterface;
use lightning_interfaces::reputation::{
    ReputationAggregatorInterface,
    ReputationReporterInterface,
};
use lightning_interfaces::signer::SignerInterface;
use lightning_interfaces::types::{Block, NodePorts, UpdateMethod, UpdatePayload, UpdateRequest};
use lightning_interfaces::{
    partial,
    ForwarderInterface,
    KeystoreInterface,
    ReputationQueryInteface,
    SyncQueryRunnerInterface,
    ToDigest,
    Weight,
};
use lightning_notifier::Notifier;
use lightning_signer::Signer;
use lightning_test_utils::consensus::{Config as ConsensusConfig, MockConsensus, MockForwarder};
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_utils::application::QueryRunnerExt;
use tokio::sync::mpsc;

use crate::aggregator::ReputationAggregator;
use crate::config::Config;
use crate::measurement_manager::Interactions;

partial!(TestBinding {
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
    let keystore = EphemeralKeystore::default();
    let (consensus_secret_key, node_secret_key) =
        (keystore.get_bls_sk(), keystore.get_ed25519_sk());
    let (consensus_public_key, node_public_key) =
        (consensus_secret_key.to_pk(), node_secret_key.to_pk());
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();

    let mut genesis = Genesis::load().unwrap();

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

    let app = Application::<TestBinding>::init(
        AppConfig {
            genesis: Some(genesis),
            mode: Mode::Test,
            testnet: false,
            storage: StorageConfig::InMemory,
            db_path: None,
            db_options: None,
        },
        Default::default(),
    )
    .unwrap();
    app.start().await;

    let (update_socket, query_runner) = (app.transaction_executor(), app.sync_query());

    let forwarder = MockForwarder::<TestBinding>::init(
        Default::default(),
        consensus_public_key,
        query_runner.clone(),
    )
    .unwrap();

    let mut signer = Signer::<TestBinding>::init(
        Default::default(),
        keystore.clone(),
        query_runner.clone(),
        forwarder.mempool_socket(),
    )
    .unwrap();

    let notifier = Notifier::<TestBinding>::init(&app);

    let consensus_config = ConsensusConfig {
        min_ordering_time: 0,
        max_ordering_time: 1,
        probability_txn_lost: 0.0,
        transactions_to_lose: HashSet::new(),
        new_block_interval: Duration::from_secs(5),
    };

    let consensus = MockConsensus::<TestBinding>::init(
        consensus_config,
        keystore,
        &signer,
        update_socket,
        query_runner.clone(),
        infusion::Blank::default(),
        None,
        &notifier,
    )
    .unwrap();

    let (new_block_tx, new_block_rx) = mpsc::channel(10);

    signer.provide_new_block_notify(new_block_rx);
    notifier.notify_on_new_block(new_block_tx);

    signer.start().await;
    consensus.start().await;

    let notifier = Notifier::<TestBinding>::init(&app);
    let config = Config {
        reporter_buffer_size: 1,
    };
    let rep_aggregator = ReputationAggregator::<TestBinding>::init(
        config,
        signer.get_socket(),
        notifier,
        query_runner,
    )
    .unwrap();

    let rep_reporter = rep_aggregator.get_reporter();
    let rep_query = rep_aggregator.get_query();
    rep_aggregator.start().await;
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
}

#[tokio::test]
async fn test_submit_measurements() {
    let keystore = EphemeralKeystore::default();
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

    let mut genesis = Genesis::load().unwrap();

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

    let app = Application::<TestBinding>::init(
        AppConfig {
            genesis: Some(genesis),
            mode: Mode::Test,
            testnet: false,
            storage: StorageConfig::InMemory,
            db_path: None,
            db_options: None,
        },
        Default::default(),
    )
    .unwrap();
    app.start().await;

    let (update_socket, query_runner) = (app.transaction_executor(), app.sync_query());

    let forwarder = MockForwarder::<TestBinding>::init(
        Default::default(),
        consensus_public_key,
        query_runner.clone(),
    )
    .unwrap();

    let mut signer = Signer::<TestBinding>::init(
        Default::default(),
        keystore.clone(),
        query_runner.clone(),
        forwarder.mempool_socket(),
    )
    .unwrap();

    let notifier = Notifier::<TestBinding>::init(&app);

    let consensus_config = ConsensusConfig {
        min_ordering_time: 0,
        max_ordering_time: 1,
        probability_txn_lost: 0.0,
        transactions_to_lose: HashSet::new(),
        new_block_interval: Duration::from_secs(5),
    };
    let consensus = MockConsensus::<TestBinding>::init(
        consensus_config,
        keystore,
        &signer,
        update_socket,
        query_runner.clone(),
        infusion::Blank::default(),
        None,
        &notifier,
    )
    .unwrap();

    let (new_block_tx, new_block_rx) = mpsc::channel(10);

    signer.provide_new_block_notify(new_block_rx);
    notifier.notify_on_new_block(new_block_tx);

    signer.start().await;
    consensus.start().await;

    let notifier = Notifier::<TestBinding>::init(&app);
    let config = Config {
        reporter_buffer_size: 1,
    };
    let rep_aggregator = ReputationAggregator::<TestBinding>::init(
        config,
        signer.get_socket(),
        notifier,
        query_runner.clone(),
    )
    .unwrap();

    let rep_reporter = rep_aggregator.get_reporter();
    rep_aggregator.start().await;

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
}

#[tokio::test]
async fn test_reputation_calculation_and_query() {
    // The application will only calculate reputation scores for nodes if multiple nodes
    // have reported measurements. Therefore, we need to create two reputation aggregators, two
    // signers, and two consensus services that will receive a socket for the same application
    // service.
    let keystore1 = EphemeralKeystore::default();
    let keystore2 = EphemeralKeystore::default();
    let (consensus_secret_key1, node_secret_key1) =
        (keystore1.get_bls_sk(), keystore1.get_ed25519_sk());
    let (consensus_secret_key2, node_secret_key2) =
        (keystore2.get_bls_sk(), keystore2.get_ed25519_sk());

    let (committee, mut keystores) = get_genesis_committee(4);
    let mut genesis = Genesis::load().unwrap();
    let chain_id = genesis.chain_id;

    genesis.node_info = committee;

    let node_public_key1 = node_secret_key1.to_pk();
    let consensus_public_key1 = consensus_secret_key1.to_pk();
    let owner_secret_key1 = AccountOwnerSecretKey::generate();
    let owner_public_key1 = owner_secret_key1.to_pk();

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

    let app = Application::<TestBinding>::init(
        AppConfig {
            genesis: Some(genesis),
            mode: Mode::Test,
            testnet: false,
            storage: StorageConfig::InMemory,
            db_path: None,
            db_options: None,
        },
        Default::default(),
    )
    .unwrap();
    app.start().await;

    let (update_socket, query_runner) = (app.transaction_executor(), app.sync_query());

    // mock forwarder will send to all consensus instances
    let forwarder = MockForwarder::<TestBinding>::init(
        Default::default(),
        consensus_public_key1,
        query_runner.clone(),
    )
    .unwrap();

    let mut signer1 = Signer::<TestBinding>::init(
        Default::default(),
        keystore1.clone(),
        query_runner.clone(),
        forwarder.mempool_socket(),
    )
    .unwrap();
    let mut signer2 = Signer::<TestBinding>::init(
        Default::default(),
        keystore2.clone(),
        query_runner.clone(),
        forwarder.mempool_socket(),
    )
    .unwrap();

    let notifier1 = Notifier::<TestBinding>::init(&app);
    let notifier2 = Notifier::<TestBinding>::init(&app);

    let consensus_config = ConsensusConfig {
        min_ordering_time: 0,
        max_ordering_time: 1,
        probability_txn_lost: 0.0,
        transactions_to_lose: HashSet::new(),
        new_block_interval: Duration::from_secs(5),
    };
    // Need to Clone later while we have 2 different MockConsensus
    let update_socket = update_socket;
    let consensus1 = MockConsensus::<TestBinding>::init(
        consensus_config.clone(),
        keystore1.clone(),
        &signer1,
        update_socket.clone(),
        query_runner.clone(),
        infusion::Blank::default(),
        None,
        &notifier1,
    )
    .unwrap();

    let consensus2 = MockConsensus::<TestBinding>::init(
        consensus_config,
        keystore2.clone(),
        &signer2,
        update_socket.clone(),
        query_runner.clone(),
        infusion::Blank::default(),
        None,
        &notifier2,
    )
    .unwrap();

    let (new_block_tx_1, new_block_rx_1) = mpsc::channel(10);
    signer1.provide_new_block_notify(new_block_rx_1);
    notifier1.notify_on_new_block(new_block_tx_1);

    let (new_block_tx_2, new_block_rx_2) = mpsc::channel(10);
    signer2.provide_new_block_notify(new_block_rx_2);
    notifier2.notify_on_new_block(new_block_tx_2);

    signer1.start().await;

    signer2.start().await;
    consensus1.start().await;

    consensus2.start().await;

    let notifier1 = Notifier::<TestBinding>::init(&app);

    let notifier2 = Notifier::<TestBinding>::init(&app);
    let config = Config {
        reporter_buffer_size: 1,
    };
    let rep_aggregator1 = ReputationAggregator::<TestBinding>::init(
        config.clone(),
        signer1.get_socket(),
        notifier1,
        query_runner.clone(),
    )
    .unwrap();
    let rep_aggregator2 = ReputationAggregator::<TestBinding>::init(
        config,
        signer2.get_socket(),
        notifier2,
        query_runner.clone(),
    )
    .unwrap();

    let rep_reporter1 = rep_aggregator1.get_reporter();
    let rep_reporter2 = rep_aggregator2.get_reporter();
    rep_aggregator1.start().await;
    rep_aggregator2.start().await;

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
        tokio::select! {
            _ = interval.tick() => {
                let measure_alice =
                    query_runner.get_reputation_measurements(&alice).unwrap_or(Vec::new());
                let measure_bob =
                    query_runner.get_reputation_measurements(&bob).unwrap_or(Vec::new());
                if measure_alice.len() == 2 && measure_bob.len() == 2 {
                    // Wait until all measurements were submitted to the application.
                    break;
                }
            }
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
        let _res = update_socket
            .run(Block {
                transactions: vec![req.into()],
                digest: [0; 32],
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
        .get_reputation_score(&alice)
        .expect("Reputation score for alice is missing");
    let bob_rep = query_runner
        .get_reputation_score(&bob)
        .expect("Reputation score for bob is missing");
    assert!(alice_rep > bob_rep);
}
