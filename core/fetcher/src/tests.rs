use std::path::PathBuf;
use std::time::Duration;

use cid::multihash::{Code, MultihashDigest};
use cid::Cid;
use fleek_crypto::{AccountOwnerSecretKey, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::{Config as AppConfig, Mode, StorageConfig};
use lightning_application::genesis::{Genesis, GenesisNode};
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore::config::Config as BlockstoreConfig;
use lightning_broadcast::{Broadcast, Config as BroadcastConfig};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{
    FetcherRequest,
    FetcherResponse,
    ImmutablePointer,
    NodePorts,
    OriginProvider,
    Topic,
};
use lightning_interfaces::{
    partial,
    ApplicationInterface,
    BlockStoreInterface,
    BroadcastInterface,
    ConsensusInterface,
    FetcherInterface,
    OriginProviderInterface,
    PoolInterface,
    ResolverInterface,
    SignerInterface,
    WithStartAndShutdown,
};
use lightning_origin_ipfs::config::{Gateway, Protocol};
use lightning_origin_ipfs::{Config as IPFSOriginConfig, IPFSOrigin};
use lightning_pool::config::Config as PoolConfig;
use lightning_pool::pool::Pool;
use lightning_resolver::config::Config as ResolverConfig;
use lightning_resolver::resolver::Resolver;
use lightning_signer::{Config as SignerConfig, Signer};
use lightning_test_utils::consensus::{Config as ConsensusConfig, MockConsensus};
use lightning_test_utils::ipfs_gateway::spawn_gateway;

use crate::config::Config;
use crate::fetcher::Fetcher;

partial!(TestBinding {
    OriginProviderInterface = IPFSOrigin<Self>;
    BroadcastInterface = Broadcast<Self>;
    BlockStoreInterface = Blockstore<Self>;
    SignerInterface = Signer<Self>;
    ResolverInterface = Resolver<Self>;
    ApplicationInterface = Application<Self>;
    PoolInterface = Pool<Self>;
});

async fn init_fetcher(
    ipfs_gateway_port: u16,
    test_name: &str,
) -> (
    Fetcher<TestBinding>,
    Blockstore<TestBinding>,
    Signer<TestBinding>,
    Broadcast<TestBinding>,
    MockConsensus<TestBinding>,
    IPFSOrigin<TestBinding>,
    Application<TestBinding>,
    PathBuf,
) {
    let path = std::env::temp_dir().join(test_name);
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    let blockstore_path = path.join("blockstore");
    let blockstore = Blockstore::<TestBinding>::init(BlockstoreConfig {
        root: blockstore_path.try_into().unwrap(),
    })
    .unwrap();

    let signer_config = SignerConfig::test();
    let (consensus_secret_key, node_secret_key) = signer_config.load_test_keys();
    let node_public_key = node_secret_key.to_pk();
    let consensus_public_key = consensus_secret_key.to_pk();
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();

    let mut genesis = Genesis::load().unwrap();
    genesis.node_info.push(GenesisNode::new(
        owner_public_key.into(),
        node_public_key,
        "127.0.0.1".parse().unwrap(),
        consensus_public_key,
        "127.0.0.1".parse().unwrap(),
        node_public_key,
        NodePorts {
            primary: 48000_u16,
            worker: 48101_u16,
            mempool: 48202_u16,
            rpc: 48300_u16,
            pool: 48400_u16,
            dht: 48500_u16,
            handshake: 48600_u16,
            blockstore: 48700_u16,
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
        blockstore.clone(),
        Default::default(),
    )
    .unwrap();
    app.start().await;

    let (update_socket, query_runner) = (app.transaction_executor(), app.sync_query());
    let mut signer = Signer::<TestBinding>::init(signer_config, query_runner.clone()).unwrap();
    let pool = Pool::<TestBinding>::init(PoolConfig::default(), &signer).unwrap();

    let broadcast = Broadcast::<TestBinding>::init(
        BroadcastConfig::default(),
        query_runner.clone(),
        Default::default(),
        &signer,
        Default::default(),
        Default::default(),
        &pool,
    )
    .unwrap();

    let consensus = MockConsensus::<TestBinding>::init(
        ConsensusConfig::default(),
        &signer,
        update_socket.clone(),
        query_runner,
        broadcast.get_pubsub(Topic::Consensus),
    )
    .unwrap();

    signer.provide_mempool(consensus.mempool());
    signer.provide_new_block_notify(consensus.new_block_notifier());
    signer.start().await;
    consensus.start().await;

    let resolver_path = path.join("resolver");
    let config = ResolverConfig {
        store_path: resolver_path.try_into().unwrap(),
    };
    let resolver =
        Resolver::<TestBinding>::init(config, &signer, broadcast.get_pubsub(Topic::Resolver))
            .unwrap();

    let mut ipfs_origin_config = IPFSOriginConfig::default();
    ipfs_origin_config.gateways.push(Gateway {
        protocol: Protocol::Http,
        authority: format!("127.0.0.1:{ipfs_gateway_port}"),
    });
    let ipfs_origin =
        IPFSOrigin::<TestBinding>::init(IPFSOriginConfig::default(), blockstore.clone()).unwrap();
    ipfs_origin.start().await;

    let fetcher = Fetcher::<TestBinding>::init(
        Config {
            max_concurrent_origin_requests: 3,
        },
        blockstore.clone(),
        resolver,
        &ipfs_origin,
    )
    .unwrap();
    (
        fetcher,
        blockstore,
        signer,
        broadcast,
        consensus,
        ipfs_origin,
        app,
        path,
    )
}

#[tokio::test]
async fn test_simple_origin_fetch() {
    let (fetcher, blockstore, _signer, _broadcast, _consensus, _ipfs_origin, _app, path) =
        init_fetcher(30101, "lightning-test-simple-origin-fetch").await;
    fetcher.start().await;

    let req_cid =
        Cid::try_from("bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi").unwrap();
    let pointer = ImmutablePointer {
        origin: OriginProvider::IPFS,
        uri: req_cid.to_bytes(),
    };

    let req_fut = async move {
        let socket = fetcher.get_socket();
        let response = socket.run(FetcherRequest::Put { pointer }).await.unwrap();
        let hash = match response {
            FetcherResponse::Put(Ok(hash)) => hash,
            FetcherResponse::Put(Err(e)) => panic!("Failed to fetch cid: {e:?}"),
            _ => panic!("Unexpected response"),
        };

        let bytes = blockstore.read_all_to_vec(&hash).await.unwrap();
        assert!(
            Code::try_from(req_cid.hash().code())
                .ok()
                .map(|code| &code.digest(&bytes) == req_cid.hash())
                .unwrap()
        );
    };

    tokio::select! {
        _ = spawn_gateway(30101) => {

        }
        _ = req_fut => {

        }
    }

    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
}

#[tokio::test]
async fn test_start_and_shutdown() {
    let (fetcher, _blockstore, _signer, _broadcast, _consensus, _ipfs_origin, _app, path) =
        init_fetcher(30100, "lightning-test-start-and-shutdown").await;

    assert!(!fetcher.is_running());
    fetcher.start().await;
    assert!(fetcher.is_running());
    fetcher.shutdown().await;
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(!fetcher.is_running());

    // start again
    fetcher.start().await;
    assert!(fetcher.is_running());
    fetcher.shutdown().await;
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(!fetcher.is_running());

    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
}
