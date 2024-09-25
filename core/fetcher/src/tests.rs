use std::sync::Arc;
use std::time::Duration;

use cid::Cid;
use fleek_crypto::{AccountOwnerSecretKey, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::{ApplicationConfig, StorageConfig};
use lightning_application::state::QueryRunner;
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore::config::Config as BlockstoreConfig;
use lightning_blockstore_server::BlockstoreServer;
use lightning_broadcast::Broadcast;
use lightning_indexer::Indexer;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    FetcherRequest,
    FetcherResponse,
    Genesis,
    GenesisNode,
    ImmutablePointer,
    NodePorts,
    OriginProvider,
};
use lightning_node::Node;
use lightning_notifier::Notifier;
use lightning_origin_ipfs::config::{Gateway, Protocol, RequestFormat};
use lightning_origin_ipfs::Config as IPFSOriginConfig;
use lightning_pool::{Config as PoolConfig, PoolProvider};
use lightning_rep_collector::aggregator::ReputationAggregator;
use lightning_rep_collector::config::Config as RepCollConfig;
use lightning_resolver::config::Config as ResolverConfig;
use lightning_resolver::resolver::Resolver;
use lightning_signer::Signer;
use lightning_test_utils::consensus::{
    MockConsensus,
    MockConsensusConfig,
    MockConsensusGroup,
    MockForwarder,
};
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_test_utils::server::spawn_server;
use lightning_topology::Topology;
use tempfile::{tempdir, TempDir};
use types::HandshakePorts;

use crate::config::Config;
use crate::fetcher::Fetcher;

partial_node_components!(TestBinding {
    ConfigProviderInterface = JsonConfigProvider;
    FetcherInterface = Fetcher<Self>;
    ForwarderInterface = MockForwarder<Self>;
    BroadcastInterface = Broadcast<Self>;
    BlockstoreInterface = Blockstore<Self>;
    BlockstoreServerInterface = BlockstoreServer<Self>;
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

async fn get_fetchers(
    temp_dir: &TempDir,
    gateway_port: u16,
    num_peers: usize,
) -> Vec<Node<TestBinding>> {
    let keystores = (0..num_peers)
        .map(|_| EphemeralKeystore::<TestBinding>::default())
        .collect::<Vec<_>>();
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();

    let mut genesis = Genesis {
        node_info: keystores
            .iter()
            .map(|keystore| {
                GenesisNode::new(
                    owner_public_key.into(),
                    keystore.get_ed25519_pk(),
                    "127.0.0.1".parse().unwrap(),
                    keystore.get_bls_pk(),
                    "127.0.0.1".parse().unwrap(),
                    keystore.get_ed25519_pk(),
                    NodePorts {
                        primary: 0,
                        worker: 0,
                        mempool: 0,
                        rpc: 0,
                        pool: 0,
                        pinger: 0,
                        handshake: HandshakePorts {
                            http: 0,
                            webrtc: 0,
                            webtransport: 0,
                        },
                    },
                    None,
                    true,
                )
            })
            .collect(),

        ..Default::default()
    };

    let consensus_group_start = Arc::new(tokio::sync::Notify::new());
    let consensus_group = MockConsensusGroup::new::<QueryRunner>(
        MockConsensusConfig::default(),
        None,
        Some(consensus_group_start.clone()),
    );
    let peers = keystores
        .iter()
        .enumerate()
        .map(|(i, keystore)| {
            Node::<TestBinding>::init_with_provider(
                fdi::Provider::default()
                    .with(consensus_group.clone())
                    .with(keystore.clone())
                    .with(
                        JsonConfigProvider::default()
                            .with::<Application<TestBinding>>(ApplicationConfig {
                                network: None,
                                genesis_path: None,
                                storage: StorageConfig::InMemory,
                                db_path: None,
                                db_options: None,
                                dev: None,
                            })
                            .with::<PoolProvider<TestBinding>>(PoolConfig {
                                max_idle_timeout: Duration::from_secs(5),
                                address: "0.0.0.0:0".parse().unwrap(),
                                ..Default::default()
                            })
                            .with::<ReputationAggregator<TestBinding>>(RepCollConfig {
                                reporter_buffer_size: 1,
                            })
                            .with::<Resolver<TestBinding>>(ResolverConfig {
                                store_path: temp_dir
                                    .path()
                                    .join(format!("node-{i}/resolver"))
                                    .try_into()
                                    .unwrap(),
                            })
                            .with::<Blockstore<TestBinding>>(BlockstoreConfig {
                                root: temp_dir
                                    .path()
                                    .join(format!("node-{i}/store"))
                                    .try_into()
                                    .unwrap(),
                            })
                            .with::<OriginDemuxer<TestBinding>>(DemuxerOriginConfig {
                                ipfs: IPFSOriginConfig {
                                    gateways: vec![Gateway {
                                        protocol: Protocol::Http,
                                        authority: format!("127.0.0.1:{gateway_port}"),
                                        request_format: RequestFormat::CidLast,
                                    }],
                                    gateway_timeout: Duration::from_millis(5000),
                                },
                                ..Default::default()
                            })
                            .with::<Fetcher<TestBinding>>(Config {
                                max_conc_origin_req: 3,
                            }),
                    ),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();

    // Start the nodes.
    for (i, peer) in peers.iter().enumerate() {
        peer.start().await;

        // Wait for the pool to be ready and listening.
        let pool = peer.provider.get::<PoolProvider<TestBinding>>();
        let ready = pool.wait_for_ready().await;

        // Update genesis node with the pool listening port.
        genesis.node_info[i].ports.pool = ready.listen_address.unwrap().port();
    }

    // Apply the genesis to each node.
    for peer in &peers {
        let app = peer.provider.get::<Application<TestBinding>>();
        app.apply_genesis(genesis.clone()).await.unwrap();
    }

    // Notify the mock consensus group that it can start.
    consensus_group_start.notify_waiters();

    peers
}

#[tokio::test]
async fn test_simple_origin_fetch() {
    let listen_port = spawn_server(0).unwrap();

    let temp_dir = tempdir().unwrap();
    let peers = get_fetchers(&temp_dir, listen_port, 1).await;

    let req_cid =
        Cid::try_from("bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi").unwrap();
    let pointer = ImmutablePointer {
        origin: OriginProvider::IPFS,
        uri: req_cid.to_bytes(),
    };

    let socket = peers[0].provider.get::<Fetcher<TestBinding>>().get_socket();

    let response = socket.run(FetcherRequest::Put { pointer }).await.unwrap();
    let hash = match response {
        FetcherResponse::Put(Ok(hash)) => hash,
        FetcherResponse::Put(Err(e)) => panic!("Failed to put cid: {e:?}"),
        _ => panic!("Unexpected response"),
    };

    let target_hash = [
        98, 198, 247, 73, 200, 10, 39, 129, 58, 132, 6, 107, 146, 166, 253, 195, 127, 216, 55, 121,
        191, 157, 100, 241, 241, 163, 105, 44, 243, 167, 223, 189,
    ];
    assert_eq!(hash, target_hash);

    for mut peer in peers {
        peer.shutdown().await;
    }
}

#[tokio::test]
async fn test_fetch_from_peer() {
    let listen_port = spawn_server(0).unwrap();

    let temp_dir = tempdir().unwrap();
    let mut peers = get_fetchers(&temp_dir, listen_port, 2).await;
    let mut peer1 = peers.pop().unwrap();
    let mut peer2 = peers.pop().unwrap();
    let blockstore1 = peer1.provider.get::<Blockstore<TestBinding>>().clone();
    let blockstore2 = peer2.provider.get::<Blockstore<TestBinding>>().clone();
    let socket1 = peer1.provider.get::<Fetcher<TestBinding>>().get_socket();
    let socket2 = peer2.provider.get::<Fetcher<TestBinding>>().get_socket();

    let req_cid =
        Cid::try_from("bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi").unwrap();
    let pointer = ImmutablePointer {
        origin: OriginProvider::IPFS,
        uri: req_cid.to_bytes(),
    };

    // Put some data onto peer1.

    let response = socket1.run(FetcherRequest::Put { pointer }).await.unwrap();
    let hash = match response {
        FetcherResponse::Put(Ok(hash)) => hash,
        FetcherResponse::Put(Err(e)) => panic!("Failed to put hash: {e:?}"),
        _ => panic!("Unexpected response"),
    };

    // Wait for peer1 to broadcast the record.
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Send a fetch request to peer2.
    // We don't start the corresponding dummy ipfs gateway to ensure that peer2 can only fetch the
    // content from peer1.
    let response = socket2.run(FetcherRequest::Fetch { hash }).await.unwrap();
    match response {
        FetcherResponse::Fetch(Ok(())) => {
            let content1 = blockstore1.read_all_to_vec(&hash).await.unwrap();
            let content2 = blockstore2.read_all_to_vec(&hash).await.unwrap();
            assert_eq!(content1, content2);
        },
        FetcherResponse::Fetch(Err(e)) => panic!("Failed to fetch hash: {e:?}"),
        _ => panic!("Unexpected response"),
    }

    peer1.shutdown().await;
    peer2.shutdown().await;
}
