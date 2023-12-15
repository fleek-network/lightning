use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use fleek_crypto::{AccountOwnerSecretKey, ConsensusSecretKey, NodeSecretKey, SecretKey};
use ink_quill::ToDigest;
use lightning_application::app::Application;
use lightning_application::config::{Config as AppConfig, Mode, StorageConfig};
use lightning_application::genesis::{Genesis, GenesisNode};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::schema::broadcast::{BroadcastFrame, BroadcastMessage};
use lightning_interfaces::types::{NodeIndex, NodePorts, Topic};
use lightning_interfaces::{
    partial,
    ApplicationInterface,
    BroadcastInterface,
    NotifierInterface,
    PoolInterface,
    PubSub,
    ReputationAggregatorInterface,
    SignerInterface,
    SyncQueryRunnerInterface,
    TopologyInterface,
    WithStartAndShutdown,
};
use lightning_notifier::Notifier;
use lightning_pool::{muxer, Config as PoolConfig, Pool};
use lightning_rep_collector::ReputationAggregator;
use lightning_signer::{utils, Config as SignerConfig, Signer};
use lightning_topology::{Config as TopologyConfig, Topology};
use tokio::sync::oneshot;

use crate::{Broadcast, Config};

partial!(TestBinding {
    ApplicationInterface = Application<Self>;
    PoolInterface = Pool<Self>;
    SignerInterface = Signer<Self>;
    NotifierInterface = Notifier<Self>;
    TopologyInterface = Topology<Self>;
    ReputationAggregatorInterface = ReputationAggregator<Self>;
    BroadcastInterface = Broadcast<Self>;
});

pub struct Peer<C: Collection> {
    _rep_aggregator: C::ReputationAggregatorInterface,
    _notifier: C::NotifierInterface,
    pool: C::PoolInterface,
    broadcast: C::BroadcastInterface,
    pub node_secret_key: NodeSecretKey,
    pub node_index: NodeIndex,
}

async fn get_broadcasts(
    test_name: &str,
    port_offset: u16,
    num_peers: usize,
) -> (Vec<Peer<TestBinding>>, Application<TestBinding>, PathBuf) {
    let mut signers_configs = Vec::new();
    let mut genesis = Genesis::load().unwrap();
    let path = std::env::temp_dir()
        .join("lightning-broadcast-test")
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
                pinger: 48600_u16,
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
        let peer = create_peer(&app, signer_config, address, true).await;
        peers.push(peer);
    }

    (peers, app, path)
}

async fn create_peer(
    app: &Application<TestBinding>,
    signer_config: SignerConfig,
    address: SocketAddr,
    in_state: bool,
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
    rep_aggregator.start().await;
    let config = PoolConfig {
        max_idle_timeout: Duration::from_secs(5),
        address,
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

    let node_public_key = signer.get_ed25519_pk();
    let node_index = if in_state {
        query_runner.pubkey_to_index(&node_public_key).unwrap()
    } else {
        u32::MAX
    };

    let config = Config {};

    let broadcast = Broadcast::<TestBinding>::init(
        config,
        query_runner,
        &signer,
        rep_aggregator.get_reporter(),
        &pool,
    )
    .unwrap();

    let (_, node_secret_key) = signer.get_sk();

    Peer::<TestBinding> {
        _rep_aggregator: rep_aggregator,
        _notifier: notifier,
        pool,
        broadcast,
        node_secret_key,
        node_index,
    }
}

#[tokio::test]
async fn test_send() {
    // Initialize three broadcasts
    let (peers, _app, path) = get_broadcasts("send", 28000, 3).await;

    for peer in &peers {
        peer.broadcast.start().await;
        peer.pool.start().await;
    }

    let pub_sub1 = peers[0]
        .broadcast
        .get_pubsub::<BroadcastFrame>(Topic::Debug);
    let mut pub_sub2 = peers[1]
        .broadcast
        .get_pubsub::<BroadcastFrame>(Topic::Debug);
    let mut pub_sub3 = peers[2]
        .broadcast
        .get_pubsub::<BroadcastFrame>(Topic::Debug);

    // Create a message from node1
    let message = BroadcastMessage {
        topic: Topic::Debug,
        originator: peers[0].node_secret_key.to_pk(),
        payload: String::from("hello").into_bytes(),
    };
    let target_signature = peers[0].node_secret_key.sign(&message.to_digest());

    // node2 listens to the broadcast and we make sure that it receives the same message that node1
    // sent out
    let (tx, rx2) = oneshot::channel();
    let target_message = message.clone();
    tokio::spawn(async move {
        match pub_sub2.recv().await.unwrap() {
            BroadcastFrame::Message { message, signature } => {
                assert_eq!(message, target_message);
                assert_eq!(signature, target_signature);
                tx.send(()).unwrap();
            },
            _ => panic!("Unexpected frame"),
        }
    });
    // The same applies for node3
    let target_message = message.clone();
    let (tx, rx3) = oneshot::channel();
    tokio::spawn(async move {
        match pub_sub3.recv().await.unwrap() {
            BroadcastFrame::Message { message, signature } => {
                assert_eq!(message, target_message);
                assert_eq!(signature, target_signature);
                tx.send(()).unwrap();
            },
            _ => panic!("Unexpected frame"),
        }
    });

    // node1 sends the message over the broadcast
    pub_sub1
        .send(
            &BroadcastFrame::Message {
                message,
                signature: target_signature,
            },
            None,
        )
        .await
        .unwrap();

    // wait until node2 and node3 received the messages before cleaning up
    rx2.await.unwrap();
    rx3.await.unwrap();

    // Clean up
    for peer in &peers {
        peer.broadcast.shutdown().await;
        peer.pool.shutdown().await;
    }
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
}
