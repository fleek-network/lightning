use std::path::PathBuf;
use std::time::Duration;

use fleek_crypto::{
    AccountOwnerSecretKey,
    ClientPublicKey,
    ConsensusSecretKey,
    EthAddress,
    SecretKey,
};
use lightning_application::app::Application;
use lightning_application::config::{Config as AppConfig, Mode, StorageConfig};
use lightning_application::genesis::{Genesis, GenesisAccount};
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore::config::Config as BlockstoreConfig;
use lightning_blockstore_server::{BlockStoreServer, Config as BlockServerConfig};
use lightning_broadcast::{Broadcast, Config as BroadcastConfig};
use lightning_fetcher::config::Config as FetcherConfig;
use lightning_fetcher::fetcher::Fetcher;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::Topic;
use lightning_interfaces::{
    partial,
    ApplicationInterface,
    BlockStoreInterface,
    BlockStoreServerInterface,
    BroadcastInterface,
    ConsensusInterface,
    FetcherInterface,
    NotifierInterface,
    OriginProviderInterface,
    PoolInterface,
    ReputationAggregatorInterface,
    ResolverInterface,
    ServiceExecutorInterface,
    SignerInterface,
    TopologyInterface,
    WithStartAndShutdown,
};
use lightning_notifier::Notifier;
use lightning_origin_demuxer::{Config as DemuxerOriginConfig, OriginDemuxer};
use lightning_origin_ipfs::config::{Gateway, Protocol};
use lightning_origin_ipfs::Config as IPFSOriginConfig;
use lightning_pool::{muxer, Config as PoolConfig, Pool};
use lightning_rep_collector::aggregator::ReputationAggregator;
use lightning_rep_collector::config::Config as RepCollConfig;
use lightning_resolver::config::Config as ResolverConfig;
use lightning_resolver::resolver::Resolver;
use lightning_signer::{Config as SignerConfig, Signer};
use lightning_test_utils::consensus::{Config as ConsensusConfig, MockConsensus};
use lightning_topology::{Config as TopologyConfig, Topology};
use tokio::sync::mpsc;

use crate::shim::{ServiceExecutor, ServiceExecutorConfig};

partial!(TestBinding {
    ServiceExecutorInterface = ServiceExecutor<Self>;
    FetcherInterface = Fetcher<Self>;
    OriginProviderInterface = OriginDemuxer<Self>;
    BroadcastInterface = Broadcast<Self>;
    BlockStoreInterface = Blockstore<Self>;
    BlockStoreServerInterface = BlockStoreServer<Self>;
    SignerInterface = Signer<Self>;
    ResolverInterface = Resolver<Self>;
    ApplicationInterface = Application<Self>;
    PoolInterface = Pool<Self>;
    NotifierInterface = Notifier<Self>;
    TopologyInterface = Topology<Self>;
    ReputationAggregatorInterface = ReputationAggregator<Self>;
});

struct Peer<C: Collection> {
    service_exec: C::ServiceExecutorInterface,
    _fetcher: C::FetcherInterface,
    _pool: C::PoolInterface,
    _broadcast: C::BroadcastInterface,
    _blockstore_server: C::BlockStoreServerInterface,
    _rep_aggregator: C::ReputationAggregatorInterface,
    _signer: C::SignerInterface,
    _origin_provider: C::OriginProviderInterface,
    _blockstore: C::BlockStoreInterface,
}

async fn init_service_executor(
    genesis: Genesis,
    path: PathBuf,
    pool_port: u16,
    gateway_port: u16,
    service_id: u32,
) -> (Peer<TestBinding>, Application<TestBinding>) {
    let blockstore = Blockstore::<TestBinding>::init(BlockstoreConfig {
        root: path.join("dummy_blockstore").try_into().unwrap(),
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
        blockstore,
    )
    .unwrap();
    app.start().await;

    let (update_socket, query_runner) = (app.transaction_executor(), app.sync_query());
    let mut signer =
        Signer::<TestBinding>::init(SignerConfig::test(), query_runner.clone()).unwrap();
    let topology = Topology::<TestBinding>::init(
        TopologyConfig::default(),
        signer.get_ed25519_pk(),
        query_runner.clone(),
    )
    .unwrap();

    let notifier = Notifier::<TestBinding>::init(&app);

    let rep_coll_config = RepCollConfig {
        reporter_buffer_size: 1,
    };
    let rep_aggregator = ReputationAggregator::<TestBinding>::init(
        rep_coll_config,
        signer.get_socket(),
        notifier.clone(),
        query_runner.clone(),
    )
    .unwrap();

    let config = PoolConfig {
        max_idle_timeout: Duration::from_secs(5),
        address: format!("0.0.0.0:{}", pool_port).parse().unwrap(),
        ..Default::default()
    };
    let pool = Pool::<TestBinding, muxer::quinn::QuinnMuxer>::init(
        config,
        &signer,
        query_runner.clone(),
        notifier.clone(),
        topology,
        rep_aggregator.get_reporter(),
    )
    .unwrap();

    let broadcast = Broadcast::<TestBinding>::init(
        BroadcastConfig::default(),
        query_runner.clone(),
        &signer,
        rep_aggregator.get_reporter(),
        &pool,
    )
    .unwrap();

    let consensus = MockConsensus::<TestBinding>::init(
        ConsensusConfig::default(),
        &signer,
        update_socket,
        query_runner.clone(),
        broadcast.get_pubsub(Topic::Consensus),
        None,
        &notifier,
    )
    .unwrap();

    signer.provide_mempool(consensus.mempool());

    let (new_block_tx, new_block_rx) = mpsc::channel(10);

    signer.provide_new_block_notify(new_block_rx);
    notifier.notify_on_new_block(new_block_tx);

    let resolver_path = path.join("resolver");
    let config = ResolverConfig {
        store_path: resolver_path.try_into().unwrap(),
    };
    let resolver = Resolver::<TestBinding>::init(
        config,
        &signer,
        broadcast.get_pubsub(Topic::Resolver),
        app.sync_query(),
    )
    .unwrap();
    resolver.start().await;

    let blockstore = Blockstore::<TestBinding>::init(BlockstoreConfig {
        root: path.join("blockstore").try_into().unwrap(),
    })
    .unwrap();

    let demuxer_config = DemuxerOriginConfig {
        ipfs: IPFSOriginConfig {
            gateways: vec![Gateway {
                protocol: Protocol::Http,
                authority: format!("127.0.0.1:{}", gateway_port),
            }],
        },
        ..Default::default()
    };
    let origin_provider =
        OriginDemuxer::<TestBinding>::init(demuxer_config, blockstore.clone()).unwrap();

    let blockstore_server = BlockStoreServer::<TestBinding>::init(
        BlockServerConfig::default(),
        blockstore.clone(),
        &pool,
        rep_aggregator.get_reporter(),
    )
    .unwrap();

    let fetcher = Fetcher::<TestBinding>::init(
        FetcherConfig {
            max_conc_origin_req: 3,
        },
        blockstore.clone(),
        &blockstore_server,
        resolver,
        &origin_provider,
    )
    .unwrap();

    std::env::set_var("BLOCKSTORE_PATH", path.join("blockstore"));
    std::env::set_var(
        "IPC_PATH",
        path.join("ipc").join(format!("service-{}", service_id)),
    );
    let config = ServiceExecutorConfig {
        services: [service_id].into_iter().collect(),
        ipc_path: path.join("ipc").try_into().unwrap(),
    };

    let service_exec = ServiceExecutor::<TestBinding>::init(
        config,
        &blockstore,
        fetcher.get_socket(),
        query_runner,
    )
    .unwrap();

    broadcast.start().await;
    origin_provider.start().await;
    signer.start().await;
    fetcher.start().await;
    pool.start().await;
    rep_aggregator.start().await;
    blockstore_server.start().await;

    let peer = Peer::<TestBinding> {
        service_exec,
        _fetcher: fetcher,
        _pool: pool,
        _broadcast: broadcast,
        _blockstore_server: blockstore_server,
        _rep_aggregator: rep_aggregator,
        _signer: signer,
        _origin_provider: origin_provider,
        _blockstore: blockstore,
    };

    (peer, app)
}

#[tokio::test]
async fn test_query_client_info() {
    let path = std::env::temp_dir().join("lightning-service-ex-test-1");
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    std::fs::create_dir_all(&path).unwrap();

    let secret_key = ConsensusSecretKey::generate();
    let client_pk = ClientPublicKey(secret_key.to_pk().0);
    let account_key = AccountOwnerSecretKey::generate();
    let address: EthAddress = account_key.to_pk().into();

    let mut genesis = Genesis::load().unwrap();
    genesis.client.insert(client_pk, address);
    genesis.account.push(GenesisAccount {
        public_key: address,
        flk_balance: 47_u32.into(),
        stables_balance: 0,
        bandwidth_balance: 27,
    });

    let (node, _app) = init_service_executor(genesis, path.clone(), 30309, 40309, 1069).await;
    node.service_exec.start().await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Start the service
    fn_sdk::ipc::init_from_env();

    // Get the client bandwidth balance
    let balance = fn_sdk::api::query_client_bandwidth_balance(client_pk).await;
    assert_eq!(balance, 27);

    // Get the client FLK balance
    let balance = fn_sdk::api::query_client_flk_balance(client_pk).await;
    assert_eq!(balance, 47);

    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
}

#[tokio::test]
async fn test_query_missing_client_info() {
    let path = std::env::temp_dir().join("lightning-service-ex-test-2");
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    std::fs::create_dir_all(&path).unwrap();

    let secret_key = ConsensusSecretKey::generate();
    let client_pk = ClientPublicKey(secret_key.to_pk().0);

    let genesis = Genesis::load().unwrap();

    let (node, _app) = init_service_executor(genesis, path.clone(), 30310, 40310, 1070).await;
    node.service_exec.start().await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Start the service
    fn_sdk::ipc::init_from_env();

    // Get the client bandwidth balance
    let balance = fn_sdk::api::query_client_bandwidth_balance(client_pk).await;
    assert_eq!(balance, 0);

    // Get the client FLK balance
    let balance = fn_sdk::api::query_client_flk_balance(client_pk).await;
    assert_eq!(balance, 0);

    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
}
