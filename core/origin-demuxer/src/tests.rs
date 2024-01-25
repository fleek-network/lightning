use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use fleek_crypto::{AccountOwnerSecretKey, ConsensusSecretKey, NodeSecretKey, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::{Config as AppConfig, Mode, StorageConfig};
use lightning_application::genesis::{Genesis, GenesisNode};
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore::config::Config as BlockstoreConfig;
use lightning_indexer::Indexer;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{ImmutablePointer, NodePorts, OriginProvider};
use lightning_interfaces::{
    partial,
    ApplicationInterface,
    BlockStoreInterface,
    ConsensusInterface,
    IndexerInterface,
    NotifierInterface,
    OriginProviderInterface,
    SignerInterface,
    WithStartAndShutdown,
};
use lightning_notifier::Notifier;
use lightning_signer::{Config as SignerConfig, Signer};
use lightning_test_utils::consensus::{Config as ConsensusConfig, MockConsensus};
use lightning_test_utils::server;
use tokio::sync::mpsc;

use crate::OriginDemuxer;

partial!(TestBinding {
    ApplicationInterface = Application<Self>;
    BlockStoreInterface = Blockstore<Self>;
    SignerInterface = Signer<Self>;
    ConsensusInterface = MockConsensus<Self>;
    IndexerInterface = Indexer<Self>;
    OriginProviderInterface = OriginDemuxer<Self>;
    NotifierInterface = Notifier<Self>;
});

struct AppState {
    _app: Application<TestBinding>,
    _signer: Signer<TestBinding>,
    _consensus: MockConsensus<TestBinding>,
    blockstore: Blockstore<TestBinding>,
    temp_dir_path: PathBuf,
}

impl Drop for AppState {
    fn drop(&mut self) {
        if self.temp_dir_path.exists() {
            std::fs::remove_dir_all(self.temp_dir_path.as_path()).unwrap();
        }
    }
}

// Todo: This is the same one used in blockstore, indexer and possbily others
// so it might be useful to create a test factory.
async fn create_app_state(test_name: String) -> AppState {
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

    let path = std::env::temp_dir().join(test_name);

    let mut blockstore = Blockstore::<TestBinding>::init(BlockstoreConfig {
        root: path.clone().try_into().unwrap(),
    })
    .unwrap();

    let app = Application::<TestBinding>::init(
        AppConfig {
            genesis: Some(genesis),
            mode: Mode::Test,
            testnet: false,
            storage: StorageConfig::InMemory,
            db_path: None,
            db_options: None,
        },
        blockstore.clone(),
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

    let indexer = Indexer::<TestBinding>::init(Default::default(), query_runner, &signer).unwrap();
    blockstore.provide_indexer(indexer);

    signer.start().await;
    consensus.start().await;

    AppState {
        _app: app,
        _signer: signer,
        _consensus: consensus,
        blockstore,
        temp_dir_path: path,
    }
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
    let state = create_app_state("test_origin_muxer".to_string()).await;

    let test_fut = async move {
        let origin =
            OriginDemuxer::<TestBinding>::init(Default::default(), state.blockstore.clone())
                .unwrap();
        origin.start().await;
        let socket = origin.get_socket();

        // When: we request content from an HTTP origin.
        let hash = socket.run(pointer_ts_file).await.unwrap().unwrap();
        let bytes = state.blockstore.read_all_to_vec(&hash).await.unwrap();
        // Then: we get the expected content.
        assert_eq!(ts_file, bytes);

        // When: we request content given from an IPFS origin.
        let hash = socket.run(pointer_ipfs_file).await.unwrap().unwrap();
        let bytes = state.blockstore.read_all_to_vec(&hash).await.unwrap();
        // Then: we get the expected content.
        assert_eq!(ipfs_file, bytes);

        // Clean up.
        origin.shutdown().await;
    };

    tokio::select! {
        biased;
        _ = server::spawn_server(31000) => {}
        _ = test_fut => {}
    }
}
