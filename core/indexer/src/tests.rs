use std::collections::HashSet;
use std::time::{Duration, SystemTime};

use fleek_crypto::{AccountOwnerSecretKey, ConsensusSecretKey, NodeSecretKey, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::{Config as AppConfig, Mode, StorageConfig};
use lightning_application::genesis::{Genesis, GenesisNode};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodePorts;
use lightning_interfaces::{
    partial,
    ApplicationInterface,
    ConsensusInterface,
    IndexerInterface,
    NotifierInterface,
    SignerInterface,
    SyncQueryRunnerInterface,
    WithStartAndShutdown,
};
use lightning_notifier::Notifier;
use lightning_signer::{Config as SignerConfig, Signer};
use lightning_test_utils::consensus::{Config as ConsensusConfig, MockConsensus};
use tokio::sync::mpsc;

use crate::Indexer;

partial!(TestBinding {
    ApplicationInterface = Application<Self>;
    SignerInterface = Signer<Self>;
    ConsensusInterface = MockConsensus<Self>;
    IndexerInterface = Indexer<Self>;
    NotifierInterface = Notifier<Self>;
});

#[tokio::test]
async fn test_submission() {
    // Given: some state.
    let signer_config = SignerConfig::test();
    let (consensus_secret_key, node_secret_key) = signer_config.load_test_keys();
    let node_public_key = node_secret_key.to_pk();
    let consensus_public_key = consensus_secret_key.to_pk();
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
            pinger: 48106,
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

    let mut signer = Signer::<TestBinding>::init(signer_config, query_runner.clone()).unwrap();

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

    // Given: our index.
    let (_, sk) = signer.get_sk();
    let us = query_runner.pubkey_to_index(&sk.to_pk()).unwrap();

    // Given: an indexer.
    let indexer =
        Indexer::<TestBinding>::init(Default::default(), query_runner.clone(), &signer).unwrap();

    // When: we register a cid.
    let cid = [0u8; 32];
    indexer.register(cid).await;

    // Then: we show up in state as a provider of that CID.
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    loop {
        tokio::select! {
            _ = interval.tick() => {
                let providers = query_runner.get_cid_providers(&cid).unwrap_or_default();
                if !providers.is_empty() {
                        assert_eq!(providers.into_iter().collect::<Vec<_>>(), vec![us]);
                        break;
                }
            }
        }
    }

    // When: we unregister the cid.
    indexer.unregister(cid).await;

    // Then: state is cleared and we don't show up anymore.
    loop {
        tokio::select! {
            _ = interval.tick() => {
                let providers = query_runner.get_cid_providers(&cid).unwrap_or_default();
                if providers.is_empty() {
                        break;
                }
            }
        }
    }
}
