use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use bytes::Bytes;
use fleek_crypto::{
    AccountOwnerSecretKey,
    ConsensusSecretKey,
    NodePublicKey,
    NodeSecretKey,
    SecretKey,
};
use futures::StreamExt;
use lightning_application::app::Application;
use lightning_application::config::{Config as AppConfig, Mode, StorageConfig};
use lightning_application::genesis::{Genesis, GenesisNode};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodePorts;
use lightning_interfaces::{
    partial,
    ApplicationInterface,
    EventHandler,
    NotifierInterface,
    PoolInterface,
    ReputationAggregatorInterface,
    Request,
    Requester,
    Responder,
    Response,
    ServiceScope,
    SignerInterface,
    SyncQueryRunnerInterface,
    TopologyInterface,
    WithStartAndShutdown,
};
use lightning_notifier::Notifier;
use lightning_rep_collector::ReputationAggregator;
use lightning_signer::{utils, Config as SignerConfig, Signer};
use lightning_topology::{Config as TopologyConfig, Topology};

use crate::{muxer, pool, Config, Pool};

partial!(TestBinding {
    ApplicationInterface = Application<Self>;
    PoolInterface = Pool<Self>;
    SignerInterface = Signer<Self>;
    NotifierInterface = Notifier<Self>;
    TopologyInterface = Topology<Self>;
    ReputationAggregatorInterface = ReputationAggregator<Self>;
});

struct Peer<C: Collection> {
    pool: C::PoolInterface,
    node_public_key: NodePublicKey,
}

async fn get_pools(
    test_name: &str,
    port_offset: u16,
    num_peers: usize,
) -> (Vec<Peer<TestBinding>>, Application<TestBinding>, PathBuf) {
    let mut signers_configs = Vec::new();
    let mut genesis = Genesis::load().unwrap();
    let path = std::env::temp_dir()
        .join("lightning-pool-test")
        .join(test_name);
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();

    genesis.node_info = vec![];

    // Create signer configs and add nodes to state.
    for i in 0..num_peers {
        let node_secret_key = NodeSecretKey::generate();
        let consensus_secret_key = ConsensusSecretKey::generate();
        let node_key_path = path.join(format!("node{i}/node.pem"));
        let consensus_key_path = path.join(format!("node{i}/cons.pem"));
        utils::save(&node_key_path, node_secret_key.encode_pem()).unwrap();
        utils::save(&consensus_key_path, consensus_secret_key.encode_pem()).unwrap();
        let signer_config = SignerConfig {
            node_key_path: node_key_path.try_into().unwrap(),
            consensus_key_path: consensus_key_path.try_into().unwrap(),
        };

        signers_configs.push(signer_config);

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
                pool: port_offset + i as u16,
                dht: 48500_u16,
                // Handshake is unused so the defaults are fine.
                handshake: Default::default(),
            },
            None,
            true,
        ));
    }

    let app = Application::<TestBinding>::init(
        AppConfig {
            genesis: Some(genesis),
            mode: Mode::Test,
            testnet: false,
            storage: StorageConfig::InMemory,
            db_path: None,
            db_options: None,
        },
        Default::default(),
    )
    .unwrap();
    app.start().await;

    // Create peers.
    let mut peers = Vec::new();
    for (i, signer_config) in signers_configs.into_iter().enumerate() {
        let address: SocketAddr = format!("0.0.0.0:{}", port_offset + i as u16)
            .parse()
            .unwrap();
        let peer = create_peer(&app, signer_config, address);
        peers.push(peer);
    }

    (peers, app, path)
}

// Create a peer that is not in state.
fn create_unknown_peer(
    path: PathBuf,
    app: &Application<TestBinding>,
    peer_index: usize,
) -> Peer<TestBinding> {
    let node_secret_key = NodeSecretKey::generate();
    let consensus_secret_key = ConsensusSecretKey::generate();
    let node_key_path = path.join(format!("node{peer_index}/node.pem"));
    let consensus_key_path = path.join(format!("node{peer_index}/cons.pem"));
    utils::save(&node_key_path, node_secret_key.encode_pem()).unwrap();
    utils::save(&consensus_key_path, consensus_secret_key.encode_pem()).unwrap();
    let signer_config = SignerConfig {
        node_key_path: node_key_path.try_into().unwrap(),
        consensus_key_path: consensus_key_path.try_into().unwrap(),
    };
    create_peer(app, signer_config, "0.0.0.0:0".parse().unwrap())
}

fn create_peer(
    app: &Application<TestBinding>,
    signer_config: SignerConfig,
    address: SocketAddr,
) -> Peer<TestBinding> {
    let (_, query_runner) = (app.transaction_executor(), app.sync_query());
    let signer = Signer::<TestBinding>::init(signer_config, query_runner.clone()).unwrap();
    let notifier = Notifier::<TestBinding>::init(app);
    let topology = Topology::<TestBinding>::init(
        TopologyConfig::default(),
        signer.get_ed25519_pk(),
        query_runner.clone(),
    )
    .unwrap();
    let rep_aggregator = ReputationAggregator::<TestBinding>::init(
        Default::default(),
        signer.get_socket(),
        notifier.clone(),
        query_runner.clone(),
    )
    .unwrap();
    let config = Config {
        max_idle_timeout: Duration::from_secs(5),
        address,
    };
    let pool = Pool::<TestBinding, muxer::quinn::QuinnMuxer>::init(
        config,
        &signer,
        query_runner,
        notifier,
        topology,
        rep_aggregator.get_reporter(),
    )
    .unwrap();

    Peer::<TestBinding> {
        pool,
        node_public_key: signer.get_ed25519_pk(),
    }
}

#[tokio::test]
async fn test_send_to_one() {
    // Given: two peers.
    let (peers, app, path) = get_pools("send_to_one", 48000, 2).await;
    let query_runner = app.sync_query();

    let node_index1 = query_runner
        .pubkey_to_index(peers[0].node_public_key)
        .unwrap();
    let node_index2 = query_runner
        .pubkey_to_index(peers[1].node_public_key)
        .unwrap();

    let event_handler1 = peers[0].pool.open_event(ServiceScope::Broadcast);
    let mut event_handler2 = peers[1].pool.open_event(ServiceScope::Broadcast);

    for peer in &peers {
        peer.pool.start().await;
    }

    // When: one of the peers sends a message to the other peer.
    let msg = Bytes::from("hello");
    event_handler1.send_to_one(node_index2, msg.clone());

    // Then: the other peer receives the message.
    let (sender, recv_msg) = event_handler2.receive().await.unwrap();
    assert_eq!(recv_msg, msg);
    assert_eq!(sender, node_index1);

    // Clean up.
    for peer in &peers {
        peer.pool.shutdown().await;
    }
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
}

#[tokio::test]
async fn test_send_to_all() {
    // Given: a list of peers that are in state and some that are not.
    let (peers, app, path) = get_pools("send_to_all", 49000, 4).await;
    let unknown_peer = create_unknown_peer(path.clone(), &app, peers.len());
    let query_runner = app.sync_query();

    // Given: we start known nodes.
    let node_index1 = query_runner
        .pubkey_to_index(peers[0].node_public_key)
        .unwrap();
    let mut event_handlers: Vec<_> = peers
        .iter()
        .map(|peer| peer.pool.open_event(ServiceScope::Broadcast))
        .collect();
    for peer in &peers {
        peer.pool.start().await;
    }

    // Given: we start an unknown node.
    let mut event_handlers_unknown_peer = unknown_peer.pool.open_event(ServiceScope::Broadcast);

    unknown_peer.pool.start().await;

    // When: one of the nodes sends a message.
    let msg = Bytes::from("hello");
    event_handlers[0].send_to_all(msg.clone(), |_| true);

    // Then: the nodes in the network receive the message.
    #[allow(clippy::needless_range_loop)]
    for i in 1..peers.len() {
        let (sender, recv_msg) = event_handlers[i].receive().await.unwrap();
        assert_eq!(recv_msg, msg);
        assert_eq!(sender, node_index1);
    }

    // Then: the unknown node does not receive the message.
    assert!(
        tokio::time::timeout(
            Duration::from_secs(5),
            event_handlers_unknown_peer.receive()
        )
        .await
        .is_err()
    );

    // Clean up.
    for peer in &peers {
        peer.pool.shutdown().await;
    }
    unknown_peer.pool.shutdown().await;
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
}

#[tokio::test]
async fn test_open_req_res() {
    // Given: two peers.
    let (peers, app, path) = get_pools("open_req_res", 50000, 2).await;
    let query_runner = app.sync_query();

    let node_index1 = query_runner
        .pubkey_to_index(peers[0].node_public_key)
        .unwrap();
    let node_index2 = query_runner
        .pubkey_to_index(peers[1].node_public_key)
        .unwrap();
    let (_requester1, mut responder1) = peers[0].pool.open_req_res(ServiceScope::BlockstoreServer);
    let (requester2, _responder2) = peers[1].pool.open_req_res(ServiceScope::BlockstoreServer);

    for peer in &peers {
        peer.pool.start().await;
    }

    // When: one of the peer sends a request to the other peer.
    let chunks = vec![
        Bytes::from("one"),
        Bytes::from("two"),
        Bytes::from("three"),
        Bytes::from("end"),
    ];
    let chunks_clone = chunks.clone();

    // Then: We receive the response.
    let sender_fut = async move {
        let (request_header, mut request) = responder1.get_next_request().await.unwrap();
        assert_eq!(request_header.peer, node_index2);
        assert_eq!(request_header.bytes, Bytes::from("a hash"));
        for chunk in chunks_clone {
            request.send(chunk).await.unwrap();
        }
    };

    // The peer that receives the request sends a response.
    let recv_fut = async move {
        let end_marker = chunks[chunks.len() - 1].clone();
        let response = requester2
            .request(node_index1, Bytes::from("a hash"))
            .await
            .unwrap();
        response.status_code().unwrap();
        let mut body = response.body();
        let mut i = 0;
        loop {
            let chunk = body.next().await.unwrap().unwrap();
            assert_eq!(chunk, chunks[i]);
            if chunk == end_marker {
                break;
            }
            i += 1;
        }
    };

    futures::join!(sender_fut, recv_fut);

    // Clean up.
    for peer in &peers {
        peer.pool.shutdown().await;
    }
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
}

#[tokio::test]
async fn test_open_req_res_unknown_peer() {
    // Give: a peer.
    let (peers, _, path) = get_pools("test_open_req_res_unknown_peer", 51000, 1).await;
    let (requester1, _responder1) = peers[0].pool.open_req_res(ServiceScope::BlockstoreServer);
    let peer = peers.first().unwrap();
    peer.pool.start().await;

    // Given: an index for an unknown node.
    let unknown_index = 6969;

    // When: we send a request to a node that is not in state.
    let response = requester1
        .request(unknown_index, Bytes::from("a hash"))
        .await;

    // Then: our request fails.
    let _expected_err: Result<pool::Response, io::Error> =
        Err(io::Error::from(io::ErrorKind::AddrNotAvailable));
    assert!(matches!(response, _expected_err));

    // Clean up.
    peer.pool.shutdown().await;
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
}
