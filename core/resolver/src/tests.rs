use std::time::Duration;

use fleek_crypto::{AccountOwnerSecretKey, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::{Config as AppConfig, Mode, StorageConfig};
use lightning_application::genesis::{Genesis, GenesisNode};
use lightning_broadcast::{Broadcast, Config as BroadcastConfig};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{NodePorts, Topic};
use lightning_interfaces::{
    partial,
    ApplicationInterface,
    BroadcastInterface,
    ConsensusInterface,
    KeystoreInterface,
    NotifierInterface,
    PoolInterface,
    ReputationAggregatorInterface,
    ResolverInterface,
    SignerInterface,
    TopologyInterface,
    WithStartAndShutdown,
};
use lightning_notifier::Notifier;
use lightning_pool::{muxer, Config as PoolConfig, PoolProvider};
use lightning_rep_collector::ReputationAggregator;
use lightning_signer::Signer;
use lightning_test_utils::consensus::{Config as ConsensusConfig, MockConsensus, MockForwarder};
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_topology::{Config as TopologyConfig, Topology};
use tokio::sync::mpsc;

use crate::config::Config;
use crate::resolver::Resolver;

partial!(TestBinding {
    ApplicationInterface = Application<Self>;
    ConsensusInterface = MockConsensus<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    SignerInterface = Signer<Self>;
    BroadcastInterface = Broadcast<Self>;
    ReputationAggregatorInterface = ReputationAggregator<Self>;
    NotifierInterface = Notifier<Self>;
    PoolInterface = PoolProvider<Self>;
});

#[tokio::test]
async fn test_start_shutdown() {
    let keystore = EphemeralKeystore::default();
    let (consensus_secret_key, node_secret_key) =
        (keystore.get_bls_sk(), keystore.get_ed25519_sk());
    let node_public_key = node_secret_key.to_pk();
    let consensus_public_key = consensus_secret_key.to_pk();
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
        keystore.get_bls_pk(),
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

    let rep_aggregator = ReputationAggregator::<TestBinding>::init(
        lightning_rep_collector::config::Config::default(),
        signer.get_socket(),
        notifier.clone(),
        query_runner.clone(),
    )
    .unwrap();

    let topology = Topology::<TestBinding>::init(
        TopologyConfig::default(),
        node_public_key,
        notifier.clone(),
        query_runner.clone(),
    )
    .unwrap();

    let pool = PoolProvider::<TestBinding, muxer::quinn::QuinnMuxer>::init(
        PoolConfig::default(),
        keystore.clone(),
        app.sync_query(),
        notifier.clone(),
        topology.get_receiver(),
        rep_aggregator.get_reporter(),
    )
    .unwrap();

    let broadcast = Broadcast::<TestBinding>::init(
        BroadcastConfig::default(),
        query_runner.clone(),
        keystore.clone(),
        rep_aggregator.get_reporter(),
        &pool,
    )
    .unwrap();

    let consensus = MockConsensus::<TestBinding>::init(
        ConsensusConfig::default(),
        keystore.clone(),
        &signer,
        update_socket,
        query_runner,
        broadcast.get_pubsub(Topic::Consensus),
        None,
        &notifier,
    )
    .unwrap();

    let (new_block_tx, new_block_rx) = mpsc::channel(10);

    signer.provide_new_block_notify(new_block_rx);
    notifier.notify_on_new_block(new_block_tx);

    signer.start().await;
    consensus.start().await;

    // Now for the actual test
    let path = std::env::temp_dir().join("resolver-test");
    if path.exists() {
        std::fs::remove_dir_all(&path).expect("Failed to clean up directory before test");
    }

    let config = Config {
        store_path: path.clone().try_into().unwrap(),
    };
    let resolver = Resolver::<TestBinding>::init(
        config,
        keystore,
        broadcast.get_pubsub(Topic::Resolver),
        app.sync_query(),
    )
    .unwrap();

    assert!(!resolver.is_running());
    resolver.start().await;
    assert!(resolver.is_running());
    // Since shutdown is no longer doing async operations we need to wait a millisecond for it to
    // finish shutting down
    resolver.shutdown().await;
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(!resolver.is_running());

    resolver.start().await;
    assert!(resolver.is_running());

    resolver.shutdown().await;
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(!resolver.is_running());

    if path.exists() {
        std::fs::remove_dir_all(&path).expect("Failed to clean up directory after test");
    }
}
