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
use lightning_interfaces::infu_collection::{Collection, Node};
use lightning_interfaces::types::NodePorts;
use lightning_interfaces::{fdi, partial, BlockstoreInterface, KeystoreInterface};
use lightning_signer::Signer;
use lightning_test_utils::consensus::{Config as ConsensusConfig, MockConsensus, MockForwarder};
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_test_utils::server;

use crate::{get_url_and_sri, HttpOrigin};

partial!(TestBinding {
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
    temp_dir_path: PathBuf,
}

impl AppState {
    fn blockstore(&self) -> fdi::Ref<Blockstore<TestBinding>> {
        self.node.provider.get()
    }
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

    let node = Node::<TestBinding>::init_with_provider(
        fdi::Provider::default()
            .with(
                JsonConfigProvider::default()
                    .with::<Blockstore<TestBinding>>(BlockstoreConfig {
                        root: path.clone().try_into().unwrap(),
                    })
                    .with::<Application<TestBinding>>(AppConfig {
                        genesis: Some(genesis),
                        mode: Mode::Test,
                        testnet: false,
                        storage: StorageConfig::InMemory,
                        db_path: None,
                        db_options: None,
                    })
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

    AppState {
        node,
        temp_dir_path: path,
    }
}

#[tokio::test]
async fn test_http_origin() {
    // Todo: let's use a different type of content.
    // Given: Some content that will be returned by gateway.
    let file: Vec<u8> = std::fs::read("../test-utils/files/index.ts").unwrap();
    // Given: an identifier for some resource.
    let url = "http://127.0.0.1:30233/bar/index.ts".to_string();
    // Given: an origin.
    let mut state = create_app_state("test_http_origin".to_string()).await;
    let origin =
        HttpOrigin::<TestBinding>::new(Default::default(), state.blockstore().clone()).unwrap();

    // When: we fetch some content using the origin.
    let test_fut = async move {
        let hash = origin.fetch(url.as_bytes()).await.unwrap();
        let bytes = state.blockstore().read_all_to_vec(&hash).await.unwrap();
        // Then: we get the expected content.
        assert_eq!(file, bytes);

        state.node.shutdown().await;
    };

    tokio::select! {
        biased;
        _ = server::spawn_server(30233) => {}
        _ = test_fut => {}
    }
}

#[tokio::test]
async fn test_http_origin_with_integrity_check() {
    // Given: Some content that will be returned by gateway.
    let file: Vec<u8> = std::fs::read("../test-utils/files/index.ts").unwrap();
    // Given: an identifier for some resource.
    let url = "http://127.0.0.1:30400/bar/index.ts#integrity=sha256-61z/GbpXJljbPypnYd2389IVCTbzU/taXTCVOUR67is=".to_string();
    // Given: an origin.
    let mut state = create_app_state("test_http_origin_with_integrity_check".to_string()).await;
    let origin =
        HttpOrigin::<TestBinding>::new(Default::default(), state.blockstore().clone()).unwrap();

    // When: we fetch some content using the origin.
    let test_fut = async move {
        let hash = origin.fetch(url.as_bytes()).await.unwrap();
        let bytes = state.blockstore().read_all_to_vec(&hash).await.unwrap();
        // Then: we get the expected content.
        assert_eq!(file, bytes);
        state.node.shutdown().await;
    };

    tokio::select! {
        biased;
        _ = server::spawn_server(30400) => {}
        _ = test_fut => {}
    }
}

#[tokio::test]
async fn test_http_origin_with_integrity_check_invalid_hash() {
    // Given: an identifier for some resource with an invalid digest.
    let url = "http://127.0.0.1:30401/bar/index.ts#integrity=sha256-23lFzBrGtqXuPufwrMw+G3hWOwdtehDz/izclz/3gVw=".to_string();
    // Given: an origin.
    let mut state =
        create_app_state("test_http_origin_with_integrity_check_invalid_hash".to_string()).await;
    let origin =
        HttpOrigin::<TestBinding>::new(Default::default(), state.blockstore().clone()).unwrap();

    // When: we fetch some content using the origin.
    let test_fut = async move {
        // Then: sri verification fails.
        assert_eq!(
            origin
                .fetch(url.as_bytes())
                .await
                .unwrap_err()
                .to_string()
                .as_str(),
            "sri failed: invalid digest"
        );

        state.node.shutdown().await;
    };

    tokio::select! {
        biased;
        _ = server::spawn_server(30401) => {}
        _ = test_fut => {}
    }
}

#[test]
fn test_url_and_integrity_hash() {
    let (_, integrity) =
        get_url_and_sri(String::from("https://lightning.com/").as_bytes()).unwrap();
    assert!(integrity.is_none());

    let (_, integrity) = get_url_and_sri(
        String::from(
            "https://lightning.com/#integrity=blake3-7eXAsQ8uxJecabUvYeQv9bQTUZzgm+DxTQmNz+X2+Y0=",
        )
        .as_bytes(),
    )
    .unwrap();
    assert_eq!(
        integrity.unwrap().to_string(),
        "blake3-7eXAsQ8uxJecabUvYeQv9bQTUZzgm+DxTQmNz+X2+Y0=".to_string()
    );

    let (_, integrity) = get_url_and_sri(
        String::from("https://lightning.com/path?bar=1&other=2#integrity=blake3-7eXAsQ8uxJecabUvYeQv9bQTUZzgm+DxTQmNz+X2+Y0=").as_bytes(),
    )
    .unwrap();
    assert_eq!(
        integrity.unwrap().to_string(),
        "blake3-7eXAsQ8uxJecabUvYeQv9bQTUZzgm+DxTQmNz+X2+Y0=".to_string()
    );

    assert!(get_url_and_sri(String::from("https://lightning.com/#integrity=").as_bytes()).is_err());
}
