use std::collections::HashSet;
use std::time::{Duration, SystemTime};

use cid::Cid;
use fleek_crypto::{AccountOwnerSecretKey, ConsensusSecretKey, NodeSecretKey, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::ApplicationConfig;
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore::config::Config as BlockstoreConfig;
use lightning_indexer::Indexer;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{Genesis, GenesisNode, NodePorts};
use lightning_node::Node;
use lightning_signer::Signer;
use lightning_test_utils::consensus::{MockConsensus, MockConsensusConfig, MockForwarder};
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_test_utils::server::spawn_server;
use tempfile::{tempdir, TempDir};

use crate::config::{Config, Gateway, Protocol, RequestFormat};
use crate::IPFSOrigin;

partial_node_components!(TestBinding {
    ConfigProviderInterface = JsonConfigProvider;
    ApplicationInterface = Application<Self>;
    BlockstoreInterface = Blockstore<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    SignerInterface = Signer<Self>;
    ForwarderInterface = MockForwarder<Self>;
    ConsensusInterface = MockConsensus<Self>;
    IndexerInterface = Indexer<Self>;
});

struct AppState {
    node: Node<TestBinding>,
}

impl AppState {
    fn blockstore(&self) -> fdi::Ref<Blockstore<TestBinding>> {
        self.node.provider.get()
    }
}

// Todo: This is the same one used in blockstore, indexer and possbily others
// so it might be useful to create a test factory.
async fn create_app_state(temp_dir: &TempDir) -> AppState {
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

    let node = Node::<TestBinding>::init_with_provider(
        fdi::Provider::default()
            .with(
                JsonConfigProvider::default()
                    .with::<Blockstore<TestBinding>>(BlockstoreConfig {
                        root: temp_dir
                            .path()
                            .join("blockstore")
                            .clone()
                            .try_into()
                            .unwrap(),
                    })
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
    .expect("failed to initialize node");

    node.start().await;

    AppState { node }
}

#[tokio::test]
async fn test_origin_dag_pb() {
    let listen_port = spawn_server(0).unwrap();

    let req_cid =
        Cid::try_from("bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi").unwrap();
    let mut config = Config::default();
    let target_bytes = std::fs::read(
        "../test-utils/files/bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi.jpeg",
    )
    .unwrap();

    let temp_dir = tempdir().unwrap();
    let mut state = create_app_state(&temp_dir).await;

    config.gateways = vec![Gateway {
        protocol: Protocol::Http,
        authority: format!("127.0.0.1:{}", listen_port),
        request_format: RequestFormat::CidLast,
    }];
    let ipfs_origin = IPFSOrigin::<TestBinding>::new(config, state.blockstore().clone()).unwrap();

    let hash = ipfs_origin
        .fetch(req_cid.to_bytes().as_slice())
        .await
        .unwrap();

    let bytes = state.blockstore().read_all_to_vec(&hash).await.unwrap();
    assert_eq!(bytes, target_bytes);

    state.node.shutdown().await;
}

#[tokio::test]
async fn test_origin_bbb_dag_pb() {
    let listen_port = spawn_server(0).unwrap();

    let req_cid =
        Cid::try_from("bafybeibi5vlbuz3jstustlxbxk7tmxsyjjrxak6us4yqq6z2df3jwidiwi").unwrap();
    let mut config = Config::default();
    let target_bytes = std::fs::read(
        "../test-utils/files/bafybeibi5vlbuz3jstustlxbxk7tmxsyjjrxak6us4yqq6z2df3jwidiwi.mp4",
    )
    .unwrap();

    let temp_dir = tempdir().unwrap();
    let mut state = create_app_state(&temp_dir).await;

    config.gateways = vec![Gateway {
        protocol: Protocol::Http,
        authority: format!("127.0.0.1:{}", listen_port),
        request_format: RequestFormat::CidLast,
    }];
    let ipfs_origin = IPFSOrigin::<TestBinding>::new(config, state.blockstore().clone()).unwrap();

    let hash = ipfs_origin
        .fetch(req_cid.to_bytes().as_slice())
        .await
        .unwrap();

    let bytes = state.blockstore().read_all_to_vec(&hash).await.unwrap();
    assert_eq!(bytes, target_bytes);

    state.node.shutdown().await;
}

#[tokio::test]
async fn test_origin_raw() {
    let listen_port = spawn_server(0).unwrap();

    let req_cid =
        Cid::try_from("bafkreihiruy5ng7d5v26c6g4gwhtastyencrefjkruqe33vwrnbyhvr74u").unwrap();
    let mut config = Config::default();
    let target_bytes = std::fs::read(
        "../test-utils/files/bafkreihiruy5ng7d5v26c6g4gwhtastyencrefjkruqe33vwrnbyhvr74u.txt",
    )
    .unwrap();

    let temp_dir = tempdir().unwrap();
    let mut state = create_app_state(&temp_dir).await;

    config.gateways = vec![Gateway {
        protocol: Protocol::Http,
        authority: format!("127.0.0.1:{}", listen_port),
        request_format: RequestFormat::CidLast,
    }];
    let ipfs_origin = IPFSOrigin::<TestBinding>::new(config, state.blockstore().clone()).unwrap();

    let hash = ipfs_origin
        .fetch(req_cid.to_bytes().as_slice())
        .await
        .unwrap();

    let bytes = state.blockstore().read_all_to_vec(&hash).await.unwrap();
    assert_eq!(bytes, target_bytes);

    state.node.shutdown().await;
}

#[tokio::test]
async fn test_origin_bbb_dag_pb_and_raw() {
    let listen_port = spawn_server(0).unwrap();

    let req_cid =
        Cid::try_from("bafybeieb3754ppknuruchkb5pxdizi5rzz42kldrps4qvjmouomyt3xkte").unwrap();
    let mut config = Config::default();
    let target_bytes = std::fs::read(
        "../test-utils/files/bafybeieb3754ppknuruchkb5pxdizi5rzz42kldrps4qvjmouomyt3xkte.js",
    )
    .unwrap();

    let temp_dir = tempdir().unwrap();
    let state = create_app_state(&temp_dir).await;

    config.gateways = vec![Gateway {
        protocol: Protocol::Http,
        authority: format!("127.0.0.1:{}", listen_port),
        request_format: RequestFormat::CidLast,
    }];
    let ipfs_origin = IPFSOrigin::<TestBinding>::new(config, state.blockstore().clone()).unwrap();

    let hash = ipfs_origin
        .fetch(req_cid.to_bytes().as_slice())
        .await
        .unwrap();

    let bytes = state.blockstore().read_all_to_vec(&hash).await.unwrap();
    assert_eq!(bytes, target_bytes);
}
