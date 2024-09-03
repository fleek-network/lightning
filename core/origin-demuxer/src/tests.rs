use std::collections::HashSet;
use std::time::{Duration, SystemTime};

use fleek_crypto::{AccountOwnerSecretKey, ConsensusSecretKey, NodeSecretKey, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::Config as AppConfig;
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore::config::Config as BlockstoreConfig;
use lightning_indexer::Indexer;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    Genesis,
    GenesisNode,
    ImmutablePointer,
    NodePorts,
    OriginProvider,
};
use lightning_signer::Signer;
use lightning_test_utils::consensus::{Config as ConsensusConfig, MockConsensus, MockForwarder};
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_test_utils::server;
use tempfile::{tempdir, TempDir};

use crate::OriginDemuxer;

partial!(TestBinding {
    ConfigProviderInterface = JsonConfigProvider;
    ApplicationInterface = Application<Self>;
    BlockstoreInterface = Blockstore<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    SignerInterface = Signer<Self>;
    ForwarderInterface = MockForwarder<Self>;
    ConsensusInterface = MockConsensus<Self>;
    IndexerInterface = Indexer<Self>;
    OriginProviderInterface = OriginDemuxer<Self>;
});

struct AppState {
    node: Node<TestBinding>,
}

impl AppState {
    fn blockstore(&self) -> fdi::Ref<Blockstore<TestBinding>> {
        self.node.provider.get()
    }

    fn demuxer(&self) -> fdi::Ref<OriginDemuxer<TestBinding>> {
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
                    .with::<Application<TestBinding>>(AppConfig::test(genesis_path))
                    .with::<MockConsensus<TestBinding>>(ConsensusConfig {
                        min_ordering_time: 0,
                        max_ordering_time: 1,
                        probability_txn_lost: 0.0,
                        transactions_to_lose: HashSet::new(),
                        new_block_interval: Duration::from_secs(5),
                    }),
            )
            .with(keystore),
    )
    .expect("failed to initialize node");

    node.start().await;

    AppState { node }
}

#[tokio::test]
async fn test_origin_muxer() {
    // Given: s ts file that will be returned by the server.
    let ts_file: Vec<u8> = std::fs::read("../test-utils/files/index.ts").unwrap();
    // Given: a pointer for that content.
    let pointer_ts_file = ImmutablePointer {
        origin: OriginProvider::HTTP,
        uri: "http://127.0.0.1:31000/bar/index.ts".as_bytes().to_vec(),
    };

    // Given: an IPFS-encoded file that will be returned by the server.
    let ipfs_file: Vec<u8> = std::fs::read(
        "../test-utils/files/bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi.car",
    )
    .unwrap();
    // Given: a pointer for that content.
    let pointer_ipfs_file = ImmutablePointer {
        origin: OriginProvider::HTTP,
        uri: "http://127.0.0.1:31000/ipfs/bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi".as_bytes().to_vec(),
    };

    // Given: some state.
    let temp_dir = tempdir().unwrap();
    let mut state = create_app_state(&temp_dir).await;

    let test_fut = async move {
        let origin = state.demuxer();
        let socket = origin.get_socket();

        // When: we request content from an HTTP origin.
        let hash = socket.run(pointer_ts_file).await.unwrap().unwrap();
        let bytes = state.blockstore().read_all_to_vec(&hash).await.unwrap();
        // Then: we get the expected content.
        assert_eq!(ts_file, bytes);

        // When: we request content given from an IPFS origin.
        let hash = socket.run(pointer_ipfs_file).await.unwrap().unwrap();
        let bytes = state.blockstore().read_all_to_vec(&hash).await.unwrap();
        // Then: we get the expected content.
        assert_eq!(ipfs_file, bytes);

        // Clean up.
        state.node.shutdown().await;
    };

    tokio::select! {
        biased;
        _ = server::spawn_server(31000) => {}
        _ = test_fut => {}
    }
}
