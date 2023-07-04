use std::time::{Duration, SystemTime};

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
    SyncQueryRunnerInterface,
};
use draco_notifier::Notifier;
use draco_signer::{Config as SignerConfig, Signer};
use draco_test_utils::consensus::{Config as ConsensusConfig, MockConsensus};
use fleek_crypto::{AccountOwnerSecretKey, NodePublicKey, PublicKey, SecretKey};

use crate::{aggregator::ReputationAggregator, config::Config};

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
        lose_every_n_txn: None,
    };
    let consensus = MockConsensus::init(
        consensus_config,
        &signer,
        update_socket.clone(),
        query_runner.clone(),
    )
    .await
    .unwrap();

    signer.provide_mempool(consensus.mempool());
    signer.provide_query_runner(query_runner.clone());
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

    let peer = NodePublicKey([1; 96]);
    rep_reporter.report_latency(&peer, Duration::from_millis(300));
    rep_reporter.report_latency(&peer, Duration::from_millis(100));
    rep_reporter.report_bytes_sent(&peer, 10_000, None);
    rep_reporter.report_bytes_received(&peer, 20_000, None);

    let mut interval = tokio::time::interval(Duration::from_millis(100));
    loop {
        tokio::select! {
            _ = &mut aggregator_handle => {}
            _ = interval.tick() => {
                let measurements = query_runner.get_rep_measurements(peer);
                if !measurements.is_empty() {
                    assert_eq!(measurements.len(), 1);
                    assert_eq!(measurements[0].reporting_node, public_key);
                    assert_eq!(
                        measurements[0].measurements.latency,
                        Some(Duration::from_millis(200))
                    );
                    assert_eq!(measurements[0].measurements.bytes_received, Some(20_000));
                    assert_eq!(measurements[0].measurements.bytes_sent, Some(10_000));
                    break;
                }
            }
        }
    }
}
