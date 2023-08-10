use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::PathBuf,
    time::Duration,
};

use fleek_crypto::{
    AccountOwnerSecretKey, NodeNetworkingSecretKey, NodeSecretKey, PublicKey, SecretKey,
};
use lightning_application::{
    app::Application,
    config::{Config as AppConfig, Mode},
    genesis::{Genesis, GenesisCommittee},
};
use lightning_interfaces::{
    application::ApplicationInterface, common::WithStartAndShutdown, consensus::ConsensusInterface,
    signer::SignerInterface, types::UpdateMethod, SyncQueryRunnerInterface,
};
use lightning_test_utils::consensus::{Config as ConsensusConfig, MockConsensus, MockPubSub};
use resolved_pathbuf::ResolvedPathBuf;

use crate::{config::Config, utils, Signer};

#[tokio::test]
async fn test_send_two_txs_in_a_row() {
    let signer_config = Config::test();
    let (secret_key, network_secret_key) = signer_config.load_test_keys();

    let mut genesis = Genesis::load().unwrap();
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

    let mut signer = Signer::init(signer_config, query_runner.clone())
        .await
        .unwrap();
    let signer_socket = signer.get_socket();

    let consensus_config = ConsensusConfig {
        min_ordering_time: 0,
        max_ordering_time: 2,
        probability_txn_lost: 0.0,
        transactions_to_lose: HashSet::new(),
        new_block_interval: Duration::from_secs(5),
    };
    let consensus = MockConsensus::init(
        consensus_config,
        &signer,
        update_socket.clone(),
        query_runner.clone(),
        MockPubSub {},
    )
    .await
    .unwrap();

    signer.provide_mempool(consensus.mempool());
    signer.provide_new_block_notify(consensus.new_block_notifier());
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
    let new_nonce = query_runner
        .get_node_info(&signer.get_bls_pk())
        .unwrap()
        .nonce;
    assert_eq!(new_nonce, 2);
}

#[tokio::test]
async fn test_retry_send() {
    let signer_config = Config::test();
    let (secret_key, network_secret_key) = signer_config.load_test_keys();
    println!("{:}", secret_key.to_pk());
    let mut genesis = Genesis::load().unwrap();

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

    let mut signer = Signer::init(signer_config, app.sync_query()).await.unwrap();

    let signer_socket = signer.get_socket();

    let consensus_config = ConsensusConfig {
        min_ordering_time: 0,
        max_ordering_time: 2,
        probability_txn_lost: 0.0,
        transactions_to_lose: HashSet::from([2]), // drop the 2nd transaction arriving
        new_block_interval: Duration::from_secs(5),
    };
    let consensus = MockConsensus::init(
        consensus_config,
        &signer,
        update_socket.clone(),
        query_runner.clone(),
        MockPubSub {},
    )
    .await
    .unwrap();

    signer.provide_mempool(consensus.mempool());
    signer.provide_new_block_notify(consensus.new_block_notifier());
    signer.start().await;
    consensus.start().await;

    // Send two transactions to the signer.
    let update_method = UpdateMethod::SubmitReputationMeasurements {
        measurements: BTreeMap::new(),
    };
    signer_socket.run(update_method).await.unwrap();
    // This transaction won't be ordered and the nonce won't be incremented on the application.
    let update_method = UpdateMethod::SubmitReputationMeasurements {
        measurements: BTreeMap::new(),
    };
    signer_socket.run(update_method).await.unwrap();
    // This transaction will have the wrong nonce, since the signer increments nonces
    // optimistically.
    let update_method = UpdateMethod::SubmitReputationMeasurements {
        measurements: BTreeMap::new(),
    };
    signer_socket.run(update_method).await.unwrap();

    // The signer will notice that the nonce doesn't increment on the application after the second
    // transaction, and then it will resend all following transactions.
    // Hence, the application nonce should be 3 after some time.
    tokio::time::sleep(Duration::from_secs(15)).await;
    let new_nonce = query_runner
        .get_node_info(&signer.get_bls_pk())
        .unwrap()
        .nonce;
    assert_eq!(new_nonce, 3);
}

#[tokio::test]
async fn test_shutdown() {
    let app = Application::init(AppConfig::default()).await.unwrap();
    let (update_socket, query_runner) = (app.transaction_executor(), app.sync_query());
    let mut signer = Signer::init(Config::default(), query_runner.clone())
        .await
        .unwrap();
    let consensus = MockConsensus::init(
        ConsensusConfig::default(),
        &signer,
        update_socket.clone(),
        query_runner.clone(),
        MockPubSub {},
    )
    .await
    .unwrap();
    signer.provide_mempool(consensus.mempool());
    signer.provide_new_block_notify(consensus.new_block_notifier());

    assert!(!signer.is_running());
    signer.start().await;
    assert!(signer.is_running());
    signer.shutdown().await;
    assert!(!signer.is_running());
}

#[tokio::test]
async fn test_sign_raw_digest() {
    let app = Application::init(AppConfig::default()).await.unwrap();
    let (update_socket, query_runner) = (app.transaction_executor(), app.sync_query());
    let mut signer = Signer::init(Config::default(), query_runner.clone())
        .await
        .unwrap();
    let consensus = MockConsensus::init(
        ConsensusConfig::default(),
        &signer,
        update_socket.clone(),
        query_runner.clone(),
        MockPubSub {},
    )
    .await
    .unwrap();
    signer.provide_mempool(consensus.mempool());
    signer.provide_new_block_notify(consensus.new_block_notifier());
    signer.start().await;

    let digest = [0; 32];
    let signature = signer.sign_raw_digest(&digest);
    let public_key = signer.get_bls_pk();
    assert!(public_key.verify(&signature, &digest));
}

#[tokio::test]
async fn test_load_keys() {
    let path = ResolvedPathBuf::try_from(PathBuf::from("~/.fleek-signer-test-1/keys"))
        .expect("Failed to resolve path");

    // Generate keys and pass the paths to the signer.
    fs::create_dir_all(&path).expect("Failed to create swarm directory");
    let node_secret_key = NodeSecretKey::generate();
    let network_secret_key = NodeNetworkingSecretKey::generate();
    let node_key_path = path.join("node.pem");
    let network_key_path = path.join("network.pem");
    utils::save(&node_key_path, node_secret_key.encode_pem()).expect("Failed to save key");
    utils::save(&network_key_path, network_secret_key.encode_pem()).expect("Failed to save key");

    let config = Config {
        node_key_path: node_key_path.try_into().expect("Failed to resolve path"),
        network_key_path: network_key_path.try_into().expect("Failed to resolve path"),
    };

    let app = Application::init(AppConfig::default()).await.unwrap();
    let (_, query_runner) = (app.transaction_executor(), app.sync_query());
    let signer = Signer::init(config, query_runner).await.unwrap();

    // Make sure that the signer loaded the keys from the provided paths.
    let (network_secret_key_loaded, node_secret_key_loaded) = signer.get_sk();
    assert_eq!(node_secret_key, node_secret_key_loaded);
    assert_eq!(network_secret_key, network_secret_key_loaded);

    fs::remove_dir_all(&path).expect("Failed to clean up signer test directory.");
}

#[tokio::test]
async fn test_fail_to_encode_keys() {
    let path = ResolvedPathBuf::try_from(PathBuf::from("~/.fleek-signer-test-2/keys"))
        .expect("Failed to resolve path");

    // Save broken keys to disk and pass the paths to the signer.
    fs::create_dir_all(&path).expect("Failed to create swarm directory");
    let node_key_path = path.join("node.pem");
    let network_key_path = path.join("network.pem");
    utils::save(&node_key_path, "I am a broken node secret key").expect("Failed to save key");
    utils::save(&network_key_path, "I am a broken network secret key").expect("Failed to save key");

    let config = Config {
        node_key_path: node_key_path.try_into().expect("Failed to resolve path"),
        network_key_path: network_key_path.try_into().expect("Failed to resolve path"),
    };

    let result = std::panic::catch_unwind(|| {
        futures::executor::block_on(async move {
            let app = Application::init(AppConfig::default()).await.unwrap();
            let (_, query_runner) = (app.transaction_executor(), app.sync_query());
            Signer::init(config, query_runner).await.unwrap();
        })
    });

    // Make sure that initializing the signer panics.
    assert!(result.is_err());

    fs::remove_dir_all(&path).expect("Failed to clean up signer test directory.");
}

#[tokio::test]
async fn test_create_keys() {
    let path = ResolvedPathBuf::try_from(PathBuf::from("~/.fleek-signer-test-3/keys"))
        .expect("Failed to resolve path");

    // Make sure this directoy doesn't exist.
    if path.is_dir() {
        fs::remove_dir_all(&path).expect("Failed to clean up signer test directory.");
    }

    // Pass the paths to the signer. No keys are in the directory.
    fs::create_dir_all(&path).expect("Failed to create swarm directory");
    let node_key_path = path.join("node.pem");
    let network_key_path = path.join("network.pem");

    let config = Config {
        node_key_path: node_key_path
            .clone()
            .try_into()
            .expect("Failed to resolve path"),
        network_key_path: network_key_path
            .clone()
            .try_into()
            .expect("Failed to resolve path"),
    };

    let app = Application::init(AppConfig::default()).await.unwrap();
    let (_, query_runner) = (app.transaction_executor(), app.sync_query());
    let signer = Signer::init(config, query_runner).await.unwrap();

    // Make sure that the signer generated keys and stored them at the specified location.
    let (network_secret_key_gen, node_secret_key_gen) = signer.get_sk();

    let node_secret_key = fs::read_to_string(&node_key_path).expect("Failed to read node pem file");
    let node_secret_key =
        NodeSecretKey::decode_pem(&node_secret_key).expect("Failed to decode node pem file");

    let network_secret_key =
        fs::read_to_string(&network_key_path).expect("Failed to read network pem file");
    let network_secret_key = NodeNetworkingSecretKey::decode_pem(&network_secret_key)
        .expect("Failed to decode network pem file");

    assert_eq!(node_secret_key, node_secret_key_gen);
    assert_eq!(network_secret_key, network_secret_key_gen);

    fs::remove_dir_all(&path).expect("Failed to clean up signer test directory.");
}
