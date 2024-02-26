use std::path::PathBuf;
use std::time::Duration;

use cid::Cid;
use fleek_crypto::{AccountOwnerSecretKey, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::{Config as AppConfig, Mode, StorageConfig};
use lightning_application::genesis::{Genesis, GenesisNode};
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore::config::Config as BlockstoreConfig;
use lightning_blockstore_server::{BlockStoreServer, Config as BlockServerConfig};
use lightning_broadcast::{Broadcast, Config as BroadcastConfig};
use lightning_indexer::Indexer;
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
    BlockStoreServerInterface,
    BroadcastInterface,
    ConsensusInterface,
    FetcherInterface,
    IndexerInterface,
    KeystoreInterface,
    NotifierInterface,
    OriginProviderInterface,
    PoolInterface,
    ReputationAggregatorInterface,
    ResolverInterface,
    SignerInterface,
    TopologyInterface,
    WithStartAndShutdown,
};
use lightning_notifier::Notifier;
use lightning_origin_demuxer::{Config as DemuxerOriginConfig, OriginDemuxer};
use lightning_origin_ipfs::config::{Gateway, Protocol};
use lightning_origin_ipfs::Config as IPFSOriginConfig;
use lightning_pool::{muxer, Config as PoolConfig, PoolProvider};
use lightning_rep_collector::aggregator::ReputationAggregator;
use lightning_rep_collector::config::Config as RepCollConfig;
use lightning_resolver::config::Config as ResolverConfig;
use lightning_resolver::resolver::Resolver;
use lightning_signer::Signer;
use lightning_test_utils::consensus::{Config as ConsensusConfig, MockConsensus};
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_test_utils::server::spawn_server;
use lightning_topology::{Config as TopologyConfig, Topology};
use tokio::sync::{mpsc, oneshot};

use crate::config::Config;
use crate::fetcher::Fetcher;

partial!(TestBinding {
    FetcherInterface = Fetcher<Self>;
    OriginProviderInterface = OriginDemuxer<Self>;
    BroadcastInterface = Broadcast<Self>;
    BlockStoreInterface = Blockstore<Self>;
    BlockStoreServerInterface = BlockStoreServer<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    SignerInterface = Signer<Self>;
    ResolverInterface = Resolver<Self>;
    ApplicationInterface = Application<Self>;
    PoolInterface = PoolProvider<Self>;
    NotifierInterface = Notifier<Self>;
    TopologyInterface = Topology<Self>;
    ConsensusInterface = MockConsensus<Self>;
    ReputationAggregatorInterface = ReputationAggregator<Self>;
    IndexerInterface = Indexer<Self>;
});

struct Peer<C: Collection> {
    fetcher: C::FetcherInterface,
    _consensus: C::ConsensusInterface,
    _pool: C::PoolInterface,
    _topology: C::TopologyInterface,
    _broadcast: C::BroadcastInterface,
    _blockstore_server: C::BlockStoreServerInterface,
    _rep_aggregator: C::ReputationAggregatorInterface,
    _signer: C::SignerInterface,
    _ipfs_origin: C::OriginProviderInterface,
    blockstore: C::BlockStoreInterface,
}

async fn get_fetchers(
    test_name: &str,
    pool_port_offset: u16,
    gateway_port_offset: u16,
    num_peers: usize,
) -> (Vec<Peer<TestBinding>>, Application<TestBinding>, PathBuf) {
    let mut keystores = Vec::new();
    let mut genesis = Genesis::load().unwrap();
    let path = std::env::temp_dir()
        .join("lightning-fetcher-test")
        .join(test_name);
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();

    genesis.node_info = vec![];

    for i in 0..num_peers {
        let keystore = EphemeralKeystore::default();
        let (consensus_secret_key, node_secret_key) =
            (keystore.get_bls_sk(), keystore.get_ed25519_sk());
        keystores.push(keystore);

        genesis.node_info.push(GenesisNode::new(
            owner_public_key.into(),
            node_secret_key.to_pk(),
            "127.0.0.1".parse().unwrap(),
            consensus_secret_key.to_pk(),
            "127.0.0.1".parse().unwrap(),
            node_secret_key.to_pk(),
            NodePorts {
                primary: 48000_u16,
                worker: 48101_u16,
                mempool: 48202_u16,
                rpc: 48300_u16,
                pool: pool_port_offset + i as u16,
                pinger: 48600_u16,
                handshake: Default::default(),
            },
            None,
            true,
        ));
    }

    // Note: This blockstore lives without an indexer. We need a signer to create an indexer.
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

    let update_socket = app.transaction_executor();
    let mut peers = Vec::new();
    for (i, keystore) in keystores.into_iter().enumerate() {
        let node_public_key = keystore.get_ed25519_pk();
        let query_runner = app.sync_query();
        let mut signer =
            Signer::<TestBinding>::init(Default::default(), keystore.clone(), query_runner.clone())
                .unwrap();
        let notifier = Notifier::<TestBinding>::init(&app);
        let topology = Topology::<TestBinding>::init(
            TopologyConfig::default(),
            node_public_key,
            notifier.clone(),
            query_runner.clone(),
        )
        .unwrap();

        let indexer = Indexer::<TestBinding>::init(
            Default::default(),
            query_runner.clone(),
            keystore.clone(),
            &signer,
        )
        .unwrap();

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
            address: format!("0.0.0.0:{}", pool_port_offset + i as u16)
                .parse()
                .unwrap(),
            ..Default::default()
        };
        let pool = PoolProvider::<TestBinding, muxer::quinn::QuinnMuxer>::init(
            config,
            keystore.clone(),
            query_runner.clone(),
            notifier.clone(),
            topology.get_receiver(),
            rep_aggregator.get_reporter(),
        )
        .unwrap();

        let broadcast = Broadcast::<TestBinding>::init(
            BroadcastConfig::default(),
            query_runner.clone(),
            keystore.clone(),
            rep_aggregator.get_reporter(),
            &pool,
        )
        .unwrap();

        let consensus = MockConsensus::<TestBinding>::init(
            ConsensusConfig::default(),
            keystore.clone(),
            &signer,
            update_socket.clone(),
            query_runner,
            broadcast.get_pubsub(Topic::Consensus),
            None,
            &notifier,
        )
        .unwrap();

        signer.provide_mempool(consensus.mempool());

        let (new_block_tx, new_block_rx) = mpsc::channel(10);

        signer.provide_new_block_notify(new_block_rx);
        notifier.notify_on_new_block(new_block_tx);

        let resolver_path = path.join(format!("node{i}/resolver"));
        let config = ResolverConfig {
            store_path: resolver_path.try_into().unwrap(),
        };
        let resolver = Resolver::<TestBinding>::init(
            config,
            keystore,
            broadcast.get_pubsub(Topic::Resolver),
            app.sync_query(),
        )
        .unwrap();
        resolver.start().await;

        let mut blockstore = Blockstore::<TestBinding>::init(BlockstoreConfig {
            root: path.join(format!("node{i}/blockstore")).try_into().unwrap(),
        })
        .unwrap();
        blockstore.provide_indexer(indexer);

        let ipfs_origin_config = IPFSOriginConfig {
            gateways: vec![Gateway {
                protocol: Protocol::Http,
                authority: format!("127.0.0.1:{}", gateway_port_offset + i as u16),
            }],
        };

        let ipfs_origin = OriginDemuxer::<TestBinding>::init(
            DemuxerOriginConfig {
                ipfs: ipfs_origin_config,
                ..Default::default()
            },
            blockstore.clone(),
        )
        .unwrap();

        let blockstore_server = BlockStoreServer::<TestBinding>::init(
            BlockServerConfig::default(),
            blockstore.clone(),
            &pool,
            rep_aggregator.get_reporter(),
        )
        .unwrap();

        let fetcher = Fetcher::<TestBinding>::init(
            Config {
                max_conc_origin_req: 3,
            },
            blockstore.clone(),
            &blockstore_server,
            resolver,
            &ipfs_origin,
        )
        .unwrap();

        broadcast.start().await;
        ipfs_origin.start().await;
        signer.start().await;
        consensus.start().await;
        fetcher.start().await;
        topology.start().await;
        pool.start().await;
        rep_aggregator.start().await;
        blockstore_server.start().await;

        let peer = Peer::<TestBinding> {
            fetcher,
            _consensus: consensus,
            _topology: topology,
            _pool: pool,
            _broadcast: broadcast,
            _blockstore_server: blockstore_server,
            _rep_aggregator: rep_aggregator,
            _signer: signer,
            _ipfs_origin: ipfs_origin,
            blockstore,
        };
        peers.push(peer);
    }

    (peers, app, path)
}

#[tokio::test]
async fn test_simple_origin_fetch() {
    let (peers, _app, path) =
        get_fetchers("lightning-test-simple-origin-fetch", 30101, 40101, 1).await;

    let req_cid =
        Cid::try_from("bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi").unwrap();
    let pointer = ImmutablePointer {
        origin: OriginProvider::IPFS,
        uri: req_cid.to_bytes(),
    };

    let req_fut = async move {
        let socket = peers[0].fetcher.get_socket();
        let response = socket.run(FetcherRequest::Put { pointer }).await.unwrap();
        let hash = match response {
            FetcherResponse::Put(Ok(hash)) => hash,
            FetcherResponse::Put(Err(e)) => panic!("Failed to put cid: {e:?}"),
            _ => panic!("Unexpected response"),
        };

        let target_hash = [
            98, 198, 247, 73, 200, 10, 39, 129, 58, 132, 6, 107, 146, 166, 253, 195, 127, 216, 55,
            121, 191, 157, 100, 241, 241, 163, 105, 44, 243, 167, 223, 189,
        ];
        assert_eq!(hash, target_hash);
    };

    tokio::select! {
        biased;
        _ = spawn_server(40101) => {

        }
        _ = req_fut => {

        }
    }

    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
}

#[tokio::test]
async fn test_fetch_from_peer() {
    let (mut peers, _app, path) =
        get_fetchers("lightning-test-fetch-from-peer", 30301, 40301, 2).await;
    let peer1 = peers.pop().unwrap();
    let peer2 = peers.pop().unwrap();
    let blockstore1 = peer1.blockstore.clone();

    let req_cid =
        Cid::try_from("bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi").unwrap();
    let pointer = ImmutablePointer {
        origin: OriginProvider::IPFS,
        uri: req_cid.to_bytes(),
    };

    // Put some data onto peer1.
    let (tx, rx) = oneshot::channel();
    let put_fut = async move {
        let socket = peer1.fetcher.get_socket();
        let response = socket.run(FetcherRequest::Put { pointer }).await.unwrap();
        let hash = match response {
            FetcherResponse::Put(Ok(hash)) => hash,
            FetcherResponse::Put(Err(e)) => panic!("Failed to put cid: {e:?}"),
            _ => panic!("Unexpected response"),
        };
        let _ = tx.send((hash, peer1));
    };
    tokio::select! {
        biased;
        _ = spawn_server(40302) => {

        }
        _ = put_fut => {

        }
    }
    let (hash, peer1) = rx.await.unwrap();

    // Wait for peer1 to broadcast the record.
    tokio::time::sleep(Duration::from_secs(4)).await;

    // Send a fetch request to peer2.
    // We don't start the corresponding dummy ipfs gateway to ensure that peer2 can only fetch the
    // content from peer1.
    let socket2 = peer2.fetcher.get_socket();
    let response = socket2.run(FetcherRequest::Fetch { hash }).await.unwrap();
    match response {
        FetcherResponse::Fetch(Ok(())) => {
            let content1 = blockstore1.read_all_to_vec(&hash).await.unwrap();
            let content2 = peer2.blockstore.read_all_to_vec(&hash).await.unwrap();
            assert_eq!(content1, content2);
        },
        FetcherResponse::Fetch(Err(e)) => panic!("Failed to fetch cid: {e:?}"),
        _ => panic!("Unexpected response"),
    }

    peer1.fetcher.shutdown().await;
    peer2.fetcher.shutdown().await;
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
}

#[tokio::test]
async fn test_start_and_shutdown() {
    let (mut peers, _app, path) =
        get_fetchers("lightning-test-start-and-shutdown", 30201, 40201, 1).await;
    let peer = peers.pop().unwrap();
    peer.fetcher.shutdown().await;
    tokio::time::sleep(Duration::from_millis(10)).await;

    assert!(!peer.fetcher.is_running());
    peer.fetcher.start().await;
    assert!(peer.fetcher.is_running());
    peer.fetcher.shutdown().await;
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(!peer.fetcher.is_running());

    // start again
    peer.fetcher.start().await;
    assert!(peer.fetcher.is_running());
    peer.fetcher.shutdown().await;
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(!peer.fetcher.is_running());

    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
}
