use std::collections::{BTreeMap, HashSet};
use std::time::Duration;

use fleek_crypto::{AccountOwnerSecretKey, NodePublicKey, PublicKey, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::{Config as AppConfig, Mode, StorageConfig};
use lightning_application::genesis::{Genesis, GenesisNode};
use lightning_interfaces::application::ApplicationInterface;
use lightning_interfaces::common::WithStartAndShutdown;
use lightning_interfaces::consensus::ConsensusInterface;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::signer::SignerInterface;
use lightning_interfaces::types::{NodePorts, UpdateMethod};
use lightning_interfaces::{
    partial,
    ForwarderInterface,
    KeystoreInterface,
    NotifierInterface,
    SyncQueryRunnerInterface,
};
use lightning_notifier::Notifier;
use lightning_test_utils::consensus::{Config as ConsensusConfig, MockConsensus, MockForwarder};
use lightning_test_utils::keys::EphemeralKeystore;
use tokio::sync::mpsc;

use crate::Signer;

partial!(TestBinding {
    ForwarderInterface = MockForwarder<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    SignerInterface = Signer<Self>;
    ApplicationInterface = Application<Self>;
    ConsensusInterface = MockConsensus<Self>;
    NotifierInterface = Notifier<Self>;
});

struct Node<C: Collection> {
    _forwarder: C::ForwarderInterface,
    _notifier: C::NotifierInterface,
    _consensus: C::ConsensusInterface,
    app: C::ApplicationInterface,
    signer: C::SignerInterface,
    node_public_key: NodePublicKey,
}

async fn init_node(consensus_config: ConsensusConfig) -> Node<TestBinding> {
    let keystore = EphemeralKeystore::default();
    let (consensus_secret_key, node_secret_key) =
        (keystore.get_bls_sk(), keystore.get_ed25519_sk());

    let mut genesis = Genesis::load().unwrap();
    let node_public_key = node_secret_key.to_pk();
    let consensus_public_key = consensus_secret_key.to_pk();
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();

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

    let consensus = MockConsensus::<TestBinding>::init(
        consensus_config,
        keystore.clone(),
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

    Node {
        _forwarder: forwarder,
        _notifier: notifier,
        _consensus: consensus,
        app,
        signer,
        node_public_key,
    }
}

#[tokio::test]
async fn test_send_two_txs_in_a_row() {
    let consensus_config = ConsensusConfig {
        min_ordering_time: 0,
        max_ordering_time: 2,
        probability_txn_lost: 0.0,
        transactions_to_lose: HashSet::new(),
        new_block_interval: Duration::from_secs(5),
    };
    let node = init_node(consensus_config).await;
    let query_runner = node.app.sync_query();

    let signer_socket = node.signer.get_socket();

    // Send two transactions to the signer.
    let update_method = UpdateMethod::SubmitReputationMeasurements {
        measurements: BTreeMap::new(),
    };
    signer_socket.run(update_method).await.unwrap();
    let update_method = UpdateMethod::SubmitReputationMeasurements {
        measurements: BTreeMap::new(),
    };
    signer_socket.run(update_method).await.unwrap();

    // Each transaction will take at most 2 seconds to get ordered.
    // Therefore, after 5 seconds, the nonce should be 2.
    tokio::time::sleep(Duration::from_secs(5)).await;
    let node_idx = query_runner.pubkey_to_index(&node.node_public_key).unwrap();
    let new_nonce = query_runner
        .get_node_info::<u64>(&node_idx, |n| n.nonce)
        .unwrap();
    assert_eq!(new_nonce, 2);
}

#[tokio::test]
async fn test_retry_send() {
    let consensus_config = ConsensusConfig {
        min_ordering_time: 0,
        max_ordering_time: 2,
        probability_txn_lost: 0.0,
        transactions_to_lose: HashSet::from([2]), // drop the 2nd transaction arriving
        new_block_interval: Duration::from_secs(5),
    };
    let node = init_node(consensus_config).await;
    let query_runner = node.app.sync_query();

    let signer_socket = node.signer.get_socket();

    // Send two transactions to the signer. The OptIn transaction was chosen arbitrarily.
    let update_method = UpdateMethod::OptIn {};
    signer_socket.run(update_method).await.unwrap();
    // This transaction won't be ordered and the nonce won't be incremented on the application.
    let update_method = UpdateMethod::OptIn {};
    signer_socket.run(update_method).await.unwrap();
    // This transaction will have the wrong nonce, since the signer increments nonces
    // optimistically.
    let update_method = UpdateMethod::OptIn {};
    signer_socket.run(update_method).await.unwrap();

    // The signer will notice that the nonce doesn't increment on the application after the second
    // transaction, and then it will resend all following transactions.
    // Hence, the application nonce should be 3 after some time.
    tokio::time::sleep(Duration::from_secs(15)).await;
    let node_idx = query_runner.pubkey_to_index(&node.node_public_key).unwrap();
    let new_nonce = query_runner
        .get_node_info::<u64>(&node_idx, |n| n.nonce)
        .unwrap();
    assert_eq!(new_nonce, 3);
}

#[tokio::test]
async fn test_shutdown() {
    let app = Application::<TestBinding>::init(AppConfig::test(), Default::default()).unwrap();
    let (_, query_runner) = (app.transaction_executor(), app.sync_query());
    let keystore = EphemeralKeystore::default();
    let forwarder = MockForwarder::<TestBinding>::init(
        Default::default(),
        [0u8; 96].into(),
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

    let (new_block_tx, new_block_rx) = mpsc::channel(10);

    signer.provide_new_block_notify(new_block_rx);
    notifier.notify_on_new_block(new_block_tx);

    assert!(!signer.is_running());
    signer.start().await;
    assert!(signer.is_running());
    signer.shutdown().await;
    // Since shutdown is no longer doing async operations we need to wait a millisecond for it to
    // finish shutting down
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(!signer.is_running());
}

#[tokio::test]
async fn test_shutdown_and_start_again() {
    let app = Application::<TestBinding>::init(AppConfig::test(), Default::default()).unwrap();
    let (_, query_runner) = (app.transaction_executor(), app.sync_query());
    let keystore = EphemeralKeystore::default();
    let forwarder = MockForwarder::<TestBinding>::init(
        Default::default(),
        [0u8; 96].into(),
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

    let (new_block_tx, new_block_rx) = mpsc::channel(10);

    signer.provide_new_block_notify(new_block_rx);
    notifier.notify_on_new_block(new_block_tx);

    assert!(!signer.is_running());
    let (new_block_tx, new_block_rx) = mpsc::channel(10);

    signer.provide_new_block_notify(new_block_rx);
    notifier.notify_on_new_block(new_block_tx);

    signer.start().await;
    assert!(signer.is_running());
    signer.shutdown().await;
    // Since shutdown is no longer doing async operations we need to wait a millisecond for it to
    // finish shutting down
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(!signer.is_running());

    let (new_block_tx, new_block_rx) = mpsc::channel(10);

    signer.provide_new_block_notify(new_block_rx);
    notifier.notify_on_new_block(new_block_tx);

    signer.start().await;
    assert!(signer.is_running());
    signer.shutdown().await;
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(!signer.is_running());
}

#[tokio::test]
async fn test_sign_raw_digest() {
    let app = Application::<TestBinding>::init(AppConfig::test(), Default::default()).unwrap();
    let (_, query_runner) = (app.transaction_executor(), app.sync_query());
    let keystore = EphemeralKeystore::default();
    let forwarder = MockForwarder::<TestBinding>::init(
        Default::default(),
        [0u8; 96].into(),
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

    let (new_block_tx, new_block_rx) = mpsc::channel(10);

    signer.provide_new_block_notify(new_block_rx);
    notifier.notify_on_new_block(new_block_tx);

    signer.start().await;

    let digest = [0; 32];
    let signature = signer.sign_raw_digest(&digest);
    let public_key = keystore.get_ed25519_pk();
    assert!(public_key.verify(&signature, &digest));
}
