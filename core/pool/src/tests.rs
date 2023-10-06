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

use crate::{muxer, Config, Pool};

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
                handshake: 48600_u16,
                blockstore: 48700_u16,
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

    let mut peers = Vec::new();
    for (i, signer_config) in signers_configs.into_iter().enumerate() {
        let (_, query_runner) = (app.transaction_executor(), app.sync_query());

        let signer = Signer::<TestBinding>::init(signer_config, query_runner.clone()).unwrap();

        let notifier = Notifier::<TestBinding>::init(&app);

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
            address: format!("0.0.0.0:{}", port_offset + i as u16)
                .parse()
                .unwrap(),
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

        let peer = Peer::<TestBinding> {
            pool,
            node_public_key: signer.get_ed25519_pk(),
        };
        peers.push(peer);
    }
    (peers, app, path)
}

#[tokio::test]
async fn test_send_to_one() {
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

    let msg = Bytes::from("hello");
    event_handler1.send_to_one(node_index2, msg.clone());
    let (sender, recv_msg) = event_handler2.receive().await.unwrap();
    assert_eq!(recv_msg, msg);
    assert_eq!(sender, node_index1);

    for peer in &peers {
        peer.pool.shutdown().await;
    }

    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
}

#[tokio::test]
async fn test_send_to_all() {
    let (peers, app, path) = get_pools("send_to_all", 49000, 4).await;
    let query_runner = app.sync_query();

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

    let msg = Bytes::from("hello");
    event_handlers[0].send_to_all(msg.clone(), |_| true);

    #[allow(clippy::needless_range_loop)]
    for i in 1..peers.len() {
        let (sender, recv_msg) = event_handlers[i].receive().await.unwrap();
        assert_eq!(recv_msg, msg);
        assert_eq!(sender, node_index1);
    }

    for peer in &peers {
        peer.pool.shutdown().await;
    }

    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
}

#[tokio::test]
async fn test_open_req_res() {
    let (peers, app, path) = get_pools("open_req_res", 49100, 2).await;
    let query_runner = app.sync_query();

    let node_index1 = query_runner
        .pubkey_to_index(peers[0].node_public_key)
        .unwrap();
    let _node_index2 = query_runner
        .pubkey_to_index(peers[1].node_public_key)
        .unwrap();
    let (_requester1, mut responder1) = peers[0].pool.open_req_res(ServiceScope::BlockstoreServer);
    let (requester2, _responder2) = peers[1].pool.open_req_res(ServiceScope::BlockstoreServer);

    for peer in &peers {
        peer.pool.start().await;
    }

    let chunks = vec![
        Bytes::from("one"),
        Bytes::from("two"),
        Bytes::from("three"),
        Bytes::from("end"),
    ];
    let chunks_clone = chunks.clone();
    let sender_fut = async move {
        let (request_bytes, mut request) = responder1.get_next_request().await.unwrap();
        assert_eq!(request_bytes, Bytes::from("a hash"));
        for chunk in chunks_clone {
            request.send(chunk).await.unwrap();
        }
    };

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

    for peer in &peers {
        peer.pool.shutdown().await;
    }

    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
}
