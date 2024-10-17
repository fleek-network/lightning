use std::time::Duration;

use cid::Cid;
use fleek_crypto::{AccountOwnerSecretKey, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::ApplicationConfig;
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
use lightning_origin_demuxer::{Config as DemuxerOriginConfig, OriginDemuxer};
use lightning_origin_ipfs::config::{Gateway, Protocol, RequestFormat};
use lightning_origin_ipfs::Config as IPFSOriginConfig;
use lightning_pool::{Config as PoolConfig, PoolProvider};
use lightning_rep_collector::aggregator::ReputationAggregator;
use lightning_rep_collector::config::Config as RepCollConfig;
use lightning_resolver::config::Config as ResolverConfig;
use lightning_resolver::resolver::Resolver;
use lightning_signer::Signer;
use lightning_test_utils::consensus::{
    Config as ConsensusConfig,
    MockConsensus,
    MockConsensusGroup,
    MockForwarder,
};
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_test_utils::server::spawn_server;
use lightning_topology::Topology;
use tempfile::{tempdir, TempDir};
use tokio::sync::oneshot;

use crate::config::Config;
use crate::fetcher::Fetcher;

partial_node_components!(TestBinding {
    ConfigProviderInterface = JsonConfigProvider;
    FetcherInterface = Fetcher<Self>;
    ForwarderInterface = MockForwarder<Self>;
    OriginProviderInterface = OriginDemuxer<Self>;
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
    pool_port_offset: u16,
    gateway_port_offset: u16,
    num_peers: usize,
) -> Vec<Node<TestBinding>> {
    let keystores = (0..num_peers)
        .map(|_| EphemeralKeystore::<TestBinding>::default())
        .collect::<Vec<_>>();
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();

    let genesis = Genesis {
        node_info: keystores
            .iter()
            .enumerate()
            .map(|(i, keystore)| {
                GenesisNode::new(
                    owner_public_key.into(),
                    keystore.get_ed25519_pk(),
                    "127.0.0.1".parse().unwrap(),
                    keystore.get_bls_pk(),
                    "127.0.0.1".parse().unwrap(),
                    keystore.get_ed25519_pk(),
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
                )
            })
            .collect(),

        ..Default::default()
    };

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let consensus_group =
        MockConsensusGroup::new::<QueryRunner>(ConsensusConfig::default(), None, None);
    let peers = keystores
        .into_iter()
        .enumerate()
        .map(|(i, keystore)| {
            Node::<TestBinding>::init_with_provider(
                fdi::Provider::default()
                    .with(consensus_group.clone())
                    .with(keystore)
                    .with(
                        JsonConfigProvider::default()
                            .with::<Application<TestBinding>>(ApplicationConfig::test(
                                genesis_path.clone(),
                            ))
                            .with::<PoolProvider<TestBinding>>(PoolConfig {
                                max_idle_timeout: Duration::from_secs(5),
                                address: format!("0.0.0.0:{}", pool_port_offset + i as u16)
                                    .parse()
                                    .unwrap(),
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
                                        authority: format!(
                                            "127.0.0.1:{}",
                                            gateway_port_offset + i as u16
                                        ),
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
        .collect();

    peers
}

#[tokio::test]
async fn test_simple_origin_fetch() {
    let temp_dir = tempdir().unwrap();
    let peers = get_fetchers(&temp_dir, 30101, 40101, 1).await;

    let req_cid =
        Cid::try_from("bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi").unwrap();
    let pointer = ImmutablePointer {
        origin: OriginProvider::IPFS,
        uri: req_cid.to_bytes(),
    };

    let socket = peers[0].provider.get::<Fetcher<TestBinding>>().get_socket();

    peers[0].start().await;

    let req_fut = async move {
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
        _ = spawn_server(40101) => {}
        _ = req_fut => {}
    }

    for mut peer in peers {
        peer.shutdown().await;
    }
}

#[tokio::test]
async fn test_fetch_from_peer() {
    let temp_dir = tempdir().unwrap();
    let mut peers = get_fetchers(&temp_dir, 30301, 40301, 2).await;
    let mut peer1 = peers.pop().unwrap();
    let mut peer2 = peers.pop().unwrap();
    let blockstore1 = peer1.provider.get::<Blockstore<TestBinding>>().clone();
    let blockstore2 = peer2.provider.get::<Blockstore<TestBinding>>().clone();
    let socket1 = peer1.provider.get::<Fetcher<TestBinding>>().get_socket();
    let socket2 = peer2.provider.get::<Fetcher<TestBinding>>().get_socket();

    peer1.start().await;
    peer2.start().await;

    let req_cid =
        Cid::try_from("bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi").unwrap();
    let pointer = ImmutablePointer {
        origin: OriginProvider::IPFS,
        uri: req_cid.to_bytes(),
    };

    // Put some data onto peer1.
    let (tx, rx) = oneshot::channel();
    let put_fut = async move {
        let response = socket1.run(FetcherRequest::Put { pointer }).await.unwrap();
        let hash = match response {
            FetcherResponse::Put(Ok(hash)) => hash,
            FetcherResponse::Put(Err(e)) => panic!("Failed to put hash: {e:?}"),
            _ => panic!("Unexpected response"),
        };
        let _ = tx.send(hash);
    };

    tokio::select! {
        biased;
        _ = spawn_server(40302) => {}
        _ = put_fut => {}
    }
    let hash = rx.await.unwrap();

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
