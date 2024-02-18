use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::time::Duration;

use fleek_crypto::{
    AccountOwnerSecretKey,
    ConsensusSecretKey,
    NodeSecretKey,
    PublicKey,
    SecretKey,
};
use lightning_application::app::Application;
use lightning_application::config::{Config as AppConfig, Mode, StorageConfig};
use lightning_application::genesis::{Genesis, GenesisNode};
use lightning_interfaces::application::ApplicationInterface;
use lightning_interfaces::common::WithStartAndShutdown;
use lightning_interfaces::consensus::ConsensusInterface;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::signer::SignerInterface;
use lightning_interfaces::types::{NodePorts, UpdateMethod};
use lightning_interfaces::{partial, NotifierInterface, SyncQueryRunnerInterface};
use lightning_notifier::Notifier;
use lightning_test_utils::consensus::{Config as ConsensusConfig, MockConsensus};
use resolved_pathbuf::ResolvedPathBuf;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::{utils, Signer};

partial!(TestBinding {
    SignerInterface = Signer<Self>;
    ApplicationInterface = Application<Self>;
    ConsensusInterface = MockConsensus<Self>;
    NotifierInterface = Notifier<Self>;
});

#[tokio::test]
async fn test_send_two_txs_in_a_row() {
    let signer_config = Config::test();
    let (consensus_secret_key, node_secret_key) = signer_config.load_test_keys();

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

    let mut signer = Signer::<TestBinding>::init(signer_config, query_runner.clone()).unwrap();
    let signer_socket = signer.get_socket();

    let notifier = Notifier::<TestBinding>::init(&app);

    let consensus_config = ConsensusConfig {
        min_ordering_time: 0,
        max_ordering_time: 2,
        probability_txn_lost: 0.0,
        transactions_to_lose: HashSet::new(),
        new_block_interval: Duration::from_secs(5),
    };

    let consensus = MockConsensus::<TestBinding>::init(
        consensus_config,
        &signer,
        update_socket,
        query_runner.clone(),
        infusion::Blank::default(),
        None,
        &notifier,
    )
    .unwrap();

    signer.provide_mempool(consensus.mempool());

    let (new_block_tx, new_block_rx) = mpsc::channel(10);

    signer.provide_new_block_notify(new_block_rx);
    notifier.notify_on_new_block(new_block_tx);

    signer.start().await;
    consensus.start().await;

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
    let node_idx = query_runner
        .pubkey_to_index(&signer.get_ed25519_pk())
        .unwrap();
    let new_nonce = query_runner
        .get_node_info::<u64>(&node_idx, |n| n.nonce)
        .unwrap();
    assert_eq!(new_nonce, 2);
}

#[tokio::test]
async fn test_retry_send() {
    let signer_config = Config::test();
    let (consensus_secret_key, node_secret_key) = signer_config.load_test_keys();

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

    let mut signer = Signer::<TestBinding>::init(signer_config, app.sync_query()).unwrap();

    let signer_socket = signer.get_socket();

    let notifier = Notifier::<TestBinding>::init(&app);

    let consensus_config = ConsensusConfig {
        min_ordering_time: 0,
        max_ordering_time: 2,
        probability_txn_lost: 0.0,
        transactions_to_lose: HashSet::from([2]), // drop the 2nd transaction arriving
        new_block_interval: Duration::from_secs(5),
    };

    let consensus = MockConsensus::<TestBinding>::init(
        consensus_config,
        &signer,
        update_socket,
        query_runner.clone(),
        infusion::Blank::default(),
        None,
        &notifier,
    )
    .unwrap();

    signer.provide_mempool(consensus.mempool());

    let (new_block_tx, new_block_rx) = mpsc::channel(10);

    signer.provide_new_block_notify(new_block_rx);
    notifier.notify_on_new_block(new_block_tx);

    signer.start().await;
    consensus.start().await;

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
    let node_idx = query_runner
        .pubkey_to_index(&signer.get_ed25519_pk())
        .unwrap();
    let new_nonce = query_runner
        .get_node_info::<u64>(&node_idx, |n| n.nonce)
        .unwrap();
    assert_eq!(new_nonce, 3);
}

#[tokio::test]
async fn test_shutdown() {
    let app = Application::<TestBinding>::init(AppConfig::test(), Default::default()).unwrap();
    let (update_socket, query_runner) = (app.transaction_executor(), app.sync_query());
    let mut signer = Signer::<TestBinding>::init(Config::test(), query_runner.clone()).unwrap();
    let notifier = Notifier::<TestBinding>::init(&app);

    let consensus = MockConsensus::<TestBinding>::init(
        ConsensusConfig::default(),
        &signer,
        update_socket,
        query_runner.clone(),
        infusion::Blank::default(),
        None,
        &notifier,
    )
    .unwrap();
    signer.provide_mempool(consensus.mempool());

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
    let (update_socket, query_runner) = (app.transaction_executor(), app.sync_query());
    let mut signer = Signer::<TestBinding>::init(Config::test(), query_runner.clone()).unwrap();
    let notifier = Notifier::<TestBinding>::init(&app);

    let consensus = MockConsensus::<TestBinding>::init(
        ConsensusConfig::default(),
        &signer,
        update_socket,
        query_runner.clone(),
        infusion::Blank::default(),
        None,
        &notifier,
    )
    .unwrap();
    signer.provide_mempool(consensus.mempool());

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
    let (update_socket, query_runner) = (app.transaction_executor(), app.sync_query());
    let mut signer = Signer::<TestBinding>::init(Config::test(), query_runner.clone()).unwrap();
    let notifier = Notifier::<TestBinding>::init(&app);

    let consensus = MockConsensus::<TestBinding>::init(
        ConsensusConfig::default(),
        &signer,
        update_socket,
        query_runner.clone(),
        infusion::Blank::default(),
        None,
        &notifier,
    )
    .unwrap();
    signer.provide_mempool(consensus.mempool());

    let (new_block_tx, new_block_rx) = mpsc::channel(10);

    signer.provide_new_block_notify(new_block_rx);
    notifier.notify_on_new_block(new_block_tx);

    signer.start().await;

    let digest = [0; 32];
    let signature = signer.sign_raw_digest(&digest);
    let public_key = signer.get_ed25519_pk();
    assert!(public_key.verify(&signature, &digest));
}

#[tokio::test]
async fn test_load_keys() {
    let path = ResolvedPathBuf::try_from("~/.lightning-signer-test-1/keys")
        .expect("Failed to resolve path");

    // Generate keys and pass the paths to the signer.
    fs::create_dir_all(&path).expect("Failed to create swarm directory");
    let node_secret_key = NodeSecretKey::generate();
    let consensus_secret_key = ConsensusSecretKey::generate();
    let node_key_path = path.join("node.pem");
    let consensus_key_path = path.join("consensus.pem");
    utils::save(&node_key_path, node_secret_key.encode_pem()).expect("Failed to save key");
    utils::save(&consensus_key_path, consensus_secret_key.encode_pem())
        .expect("Failed to save key");

    let config = Config {
        node_key_path: node_key_path.try_into().expect("Failed to resolve path"),
        consensus_key_path: consensus_key_path
            .try_into()
            .expect("Failed to resolve path"),
    };

    let app = Application::<TestBinding>::init(AppConfig::test(), Default::default()).unwrap();
    let (_, query_runner) = (app.transaction_executor(), app.sync_query());
    let signer = Signer::<TestBinding>::init(config, query_runner).unwrap();

    // Make sure that the signer loaded the keys from the provided paths.
    let (consensus_secret_key_loaded, node_secret_key_loaded) = signer.get_sk();
    assert_eq!(node_secret_key, node_secret_key_loaded);
    assert_eq!(consensus_secret_key, consensus_secret_key_loaded);

    fs::remove_dir_all(&path).expect("Failed to clean up signer test directory.");
}

#[tokio::test]
async fn test_fail_to_encode_keys() {
    let path = ResolvedPathBuf::try_from("~/.lightning-signer-test-2/keys")
        .expect("Failed to resolve path");

    // Save broken keys to disk and pass the paths to the signer.
    fs::create_dir_all(&path).expect("Failed to create swarm directory");
    let node_key_path = path.join("node.pem");
    let consensus_key_path = path.join("consensus.pem");
    utils::save(&node_key_path, "I am a broken node secret key").expect("Failed to save key");
    utils::save(&consensus_key_path, "I am a broken consensus secret key")
        .expect("Failed to save key");

    let config = Config {
        node_key_path: node_key_path.try_into().expect("Failed to resolve path"),
        consensus_key_path: consensus_key_path
            .try_into()
            .expect("Failed to resolve path"),
    };

    let result = std::panic::catch_unwind(|| {
        futures::executor::block_on(async move {
            let app =
                Application::<TestBinding>::init(AppConfig::test(), Default::default()).unwrap();
            let (_, query_runner) = (app.transaction_executor(), app.sync_query());
            Signer::<TestBinding>::init(config, query_runner).unwrap();
        })
    });

    // Make sure that initializing the signer panics.
    assert!(result.is_err());

    fs::remove_dir_all(&path).expect("Failed to clean up signer test directory.");
}

#[tokio::test]
async fn test_no_keys_exist() {
    let path = ResolvedPathBuf::try_from("~/.lightning-signer-test-3/keys")
        .expect("Failed to resolve path");

    // Make sure this directory doesn't exist.
    if path.is_dir() {
        fs::remove_dir_all(&path).expect("Failed to clean up signer test directory.");
    }

    // Pass the paths to the signer. No keys are in the directory.
    let node_key_path = path.join("node.pem");
    let consensus_key_path = path.join("consensus.pem");

    let config = Config {
        node_key_path: node_key_path.try_into().expect("Failed to resolve path"),
        consensus_key_path: consensus_key_path
            .try_into()
            .expect("Failed to resolve path"),
    };

    let app = Application::<TestBinding>::init(AppConfig::test(), Default::default()).unwrap();
    let (_, query_runner) = (app.transaction_executor(), app.sync_query());
    let signer = Signer::<TestBinding>::init(config, query_runner);

    // Initiating the signer should return an error if no keys exist.
    assert!(signer.is_err());
}

#[tokio::test]
async fn test_generate_node_key() {
    let path = ResolvedPathBuf::try_from("~/.lightning-signer-test-4/keys")
        .expect("Failed to resolve path");

    // Make sure this directory doesn't exist.
    if path.is_dir() {
        fs::remove_dir_all(&path).expect("Failed to clean up signer test directory.");
    }

    let node_key_path = path.join("node.pem");
    Signer::<TestBinding>::generate_node_key(&node_key_path).unwrap();

    // Make sure that a valid key was created.
    let node_secret_key = fs::read_to_string(&node_key_path).expect("Failed to read node pem file");
    let _ = NodeSecretKey::decode_pem(&node_secret_key).expect("Failed to decode node pem file");

    fs::remove_dir_all(&path).expect("Failed to clean up signer test directory.");
}

#[tokio::test]
async fn test_generate_consensus_key() {
    let path = ResolvedPathBuf::try_from("~/.lightning-signer-test-5/keys")
        .expect("Failed to resolve path");

    // Make sure this directory doesn't exist.
    if path.is_dir() {
        fs::remove_dir_all(&path).expect("Failed to clean up signer test directory.");
    }

    let network_key_path = path.join("consensus.pem");
    Signer::<TestBinding>::generate_consensus_key(&network_key_path).unwrap();

    // Make sure that a valid key was created.
    let network_secret_key =
        fs::read_to_string(&network_key_path).expect("Failed to read consensus pem file");
    let _ = ConsensusSecretKey::decode_pem(&network_secret_key)
        .expect("Failed to decode consensus pem file");

    fs::remove_dir_all(&path).expect("Failed to clean up signer test directory.");
}
