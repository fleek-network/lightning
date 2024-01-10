use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use fleek_crypto::{AccountOwnerSecretKey, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::{Config as AppConfig, Mode, StorageConfig};
use lightning_application::genesis::{Genesis, GenesisNode};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodePorts;
use lightning_interfaces::{
    partial,
    ApplicationInterface,
    ConsensusInterface,
    DeliveryAcknowledgmentAggregatorInterface,
    SignerInterface,
    WithStartAndShutdown,
};
use lightning_signer::{Config as SignerConfig, Signer};
use lightning_test_utils::consensus::{Config as ConsensusConfig, MockConsensus};

use crate::{Config, DeliveryAcknowledgmentAggregator};

partial!(TestBinding {
    ApplicationInterface = Application<Self>;
    ConsensusInterface = MockConsensus<Self>;
    SignerInterface = Signer<Self>;
    DeliveryAcknowledgmentAggregatorInterface = DeliveryAcknowledgmentAggregator<Self>;
});

struct Node<C: Collection> {
    _signer: C::SignerInterface,
    _app: C::ApplicationInterface,
    _consensus: C::ConsensusInterface,
    aggregator: C::DeliveryAcknowledgmentAggregatorInterface,
}

async fn init_aggregator(path: PathBuf) -> Node<TestBinding> {
    let signer_config = SignerConfig::test();
    let (consensus_secret_key, node_secret_key) = signer_config.load_test_keys();
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

    let mut signer = Signer::<TestBinding>::init(signer_config, query_runner.clone()).unwrap();

    let consensus_config = ConsensusConfig {
        min_ordering_time: 0,
        max_ordering_time: 1,
        probability_txn_lost: 0.0,
        transactions_to_lose: HashSet::new(),
        new_block_interval: Duration::from_secs(5),
    };
    let consensus = MockConsensus::<TestBinding>::init(
        consensus_config,
        &signer,
        update_socket.clone(),
        query_runner.clone(),
        infusion::Blank::default(),
        None,
    )
    .unwrap();

    signer.provide_mempool(consensus.mempool());
    signer.provide_new_block_notify(consensus.new_block_notifier());
    signer.start().await;
    consensus.start().await;

    let config = Config {
        submit_interval: Duration::from_secs(1),
        db_path: path.try_into().unwrap(),
    };

    let aggregator =
        DeliveryAcknowledgmentAggregator::<TestBinding>::init(config, signer.get_socket()).unwrap();
    Node::<TestBinding> {
        _signer: signer,
        _app: app,
        _consensus: consensus,
        aggregator,
    }
}

#[tokio::test]
async fn test_shutdown_and_start_again() {
    let path = std::env::temp_dir().join("lightning-test-dack-aggregator-shutdown");

    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    let node = init_aggregator(path.clone()).await;

    assert!(!node.aggregator.is_running());
    node.aggregator.start().await;
    assert!(node.aggregator.is_running());
    node.aggregator.shutdown().await;
    // Since shutdown is no longer doing async operations we need to wait a millisecond for it to
    // finish shutting down
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(!node.aggregator.is_running());

    node.aggregator.start().await;
    assert!(node.aggregator.is_running());
    node.aggregator.shutdown().await;
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(!node.aggregator.is_running());

    if path.exists() {
        std::fs::remove_dir_all(path).unwrap();
    }
}
