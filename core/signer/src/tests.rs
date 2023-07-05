use std::{collections::BTreeMap, sync::Arc, time::Duration};

use draco_application::{
    app::Application,
    config::{Config as AppConfig, Mode},
    genesis::{Genesis, GenesisCommittee},
};
use draco_interfaces::{
    application::ApplicationInterface, common::WithStartAndShutdown, consensus::ConsensusInterface,
    signer::SignerInterface, types::UpdateMethod, SyncQueryRunnerInterface,
};
use draco_test_utils::{
    consensus::{Config as ConsensusConfig, MockConsensus},
    empty_interfaces::MockGossip,
};
use fleek_crypto::{AccountOwnerSecretKey, PublicKey, SecretKey};

use crate::{config::Config, Signer};

#[tokio::test]
async fn test_send_two_txs_in_a_row() {
    let signer_config = Config::default();
    let mut signer = Signer::init(signer_config).await.unwrap();
    let signer_socket = signer.get_socket();

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
        max_ordering_time: 2,
        probability_txn_lost: 0.0,
        lose_every_n_txn: None,
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
    let signer_config = Config::default();
    let mut signer = Signer::init(signer_config).await.unwrap();
    let signer_socket = signer.get_socket();

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
        max_ordering_time: 2,
        probability_txn_lost: 0.0,
        lose_every_n_txn: Some(2), // drop every 2nd transaction
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
    tokio::time::sleep(Duration::from_secs(20)).await;
    let new_nonce = query_runner
        .get_node_info(&signer.get_bls_pk())
        .unwrap()
        .nonce;
    assert_eq!(new_nonce, 3);
}
