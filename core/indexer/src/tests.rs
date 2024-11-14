use std::collections::HashSet;
use std::time::{Duration, SystemTime};

use fleek_crypto::{AccountOwnerSecretKey, ConsensusSecretKey, NodeSecretKey, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::ApplicationConfig;
use lightning_application::state::QueryRunner;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{Genesis, GenesisNode, NodePorts};
use lightning_node::Node;
use lightning_notifier::Notifier;
use lightning_signer::Signer;
use lightning_test_utils::consensus::{MockConsensus, MockConsensusConfig, MockForwarder};
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use tempfile::tempdir;

use crate::Indexer;

partial_node_components!(TestBinding {
    ConfigProviderInterface = JsonConfigProvider;
    ApplicationInterface = Application<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    SignerInterface = Signer<Self>;
    ForwarderInterface = MockForwarder<Self>;
    ConsensusInterface = MockConsensus<Self>;
    IndexerInterface = Indexer<Self>;
    NotifierInterface = Notifier<Self>;
});

#[tokio::test]
async fn test_submission() {
    let temp_dir = tempdir().unwrap();

    // Given: some state.
    let keystore = EphemeralKeystore::<TestBinding>::default();
    let (consensus_secret_key, node_secret_key) =
        (keystore.get_bls_sk(), keystore.get_ed25519_sk());
    let node_public_key = node_secret_key.to_pk();
    let consensus_public_key = consensus_secret_key.to_pk();
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();

    let peer_owner_public_key = AccountOwnerSecretKey::generate();
    let peer_secret_key = NodeSecretKey::generate();
    let peer_public_key = peer_secret_key.to_pk();
    let peer_consensus_secret_key = ConsensusSecretKey::generate();
    let peer_consensus_public_key = peer_consensus_secret_key.to_pk();

    let mut genesis = Genesis::default();

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

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let mut node = Node::<TestBinding>::init_with_provider(
        fdi::Provider::default()
            .with(
                JsonConfigProvider::default()
                    .with::<Application<TestBinding>>(ApplicationConfig::test(genesis_path))
                    .with::<MockConsensus<TestBinding>>(MockConsensusConfig {
                        min_ordering_time: 0,
                        max_ordering_time: 1,
                        probability_txn_lost: 0.0,
                        transactions_to_lose: HashSet::new(),
                        new_block_interval: Duration::from_secs(5),
                        block_buffering_interval: Duration::from_secs(0),
                        forwarder_transaction_to_error: HashSet::new(),
                    }),
            )
            .with(keystore),
    )
    .unwrap();

    node.start().await;

    // Given: an indexer & query runner.
    let indexer = node.provider.get::<Indexer<TestBinding>>();
    let query_runner = node.provider.get::<QueryRunner>().clone();

    // Given: our index.
    let us = query_runner.pubkey_to_index(&node_public_key).unwrap();

    // When: we register a cid.
    let uri = [0u8; 32];
    indexer.register(uri).await;

    // Then: we show up in state as a provider of that CID.
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    loop {
        tokio::select! {
            _ = interval.tick() => {
                let providers = query_runner.get_uri_providers(&uri).unwrap_or_default();
                if !providers.is_empty() {
                        assert_eq!(providers.into_iter().collect::<Vec<_>>(), vec![us]);
                        break;
                }
            }
        }
    }

    // When: we unregister the cid.
    indexer.unregister(uri).await;

    // Then: state is cleared and we don't show up anymore.
    loop {
        tokio::select! {
            _ = interval.tick() => {
                let providers = query_runner.get_uri_providers(&uri).unwrap_or_default();
                if providers.is_empty() {
                        break;
                }
            }
        }
    }

    node.shutdown().await;
}
