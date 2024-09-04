use std::net::SocketAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fleek_crypto::{AccountOwnerSecretKey, NodeSecretKey, NodeSignature, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::Config as AppConfig;
use lightning_interfaces::prelude::*;
use lightning_interfaces::schema::broadcast::{Frame, Message};
use lightning_interfaces::types::{Genesis, GenesisNode, NodePorts, Topic};
use lightning_notifier::Notifier;
use lightning_pool::{Config as PoolConfig, PoolProvider};
use lightning_rep_collector::ReputationAggregator;
use lightning_signer::Signer;
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_topology::Topology;
use tempfile::{tempdir, TempDir};
use tokio::sync::oneshot;

use crate::Broadcast;

partial!(TestBinding {
    ConfigProviderInterface = JsonConfigProvider;
    ApplicationInterface = Application<Self>;
    PoolInterface = PoolProvider<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    SignerInterface = Signer<Self>;
    NotifierInterface = Notifier<Self>;
    TopologyInterface = Topology<Self>;
    ReputationAggregatorInterface = ReputationAggregator<Self>;
    BroadcastInterface = Broadcast<Self>;
});

pub struct Peer {
    inner: Node<TestBinding>,
    node_secret_key: NodeSecretKey,
}

impl Peer {
    fn sync_query(&self) -> fdi::Ref<c!(TestBinding::ApplicationInterface::SyncExecutor)> {
        self.inner.provider.get()
    }
    fn broadcast(&self) -> fdi::Ref<c!(TestBinding::BroadcastInterface)> {
        self.inner.provider.get()
    }
}

async fn get_broadcasts(temp_dir: &TempDir, port_offset: u16, num_peers: usize) -> Vec<Peer> {
    let mut genesis = Genesis::default();

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();

    genesis.node_info = vec![];
    let mut keystores = vec![];
    // Create signer configs and add nodes to state.
    for i in 0..num_peers {
        let keystore = EphemeralKeystore::<TestBinding>::default();
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
                pool: port_offset + i as u16,
                pinger: 48600_u16,
                // Handshake is unused so the defaults are fine.
                handshake: Default::default(),
            },
            None,
            true,
        ));
    }

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    // Create peers.
    let mut peers = Vec::new();
    for (i, keystore) in keystores.into_iter().enumerate() {
        let address: SocketAddr = format!("0.0.0.0:{}", port_offset + i as u16)
            .parse()
            .unwrap();
        let peer = create_peer(AppConfig::test(genesis_path.clone()), keystore, address).await;
        peers.push(peer);
    }

    peers
}

async fn create_peer(
    app_config: AppConfig,
    keystore: EphemeralKeystore<TestBinding>,
    address: SocketAddr,
) -> Peer {
    let node_secret_key = keystore.get_ed25519_sk();

    let inner = Node::<TestBinding>::init_with_provider(
        fdi::Provider::default()
            .with(
                JsonConfigProvider::default()
                    .with::<Application<TestBinding>>(app_config)
                    .with::<PoolProvider<TestBinding>>(PoolConfig {
                        max_idle_timeout: Duration::from_secs(5),
                        address,
                        ..Default::default()
                    }),
            )
            .with(keystore),
    )
    .expect("failed to initialize node");

    Peer {
        inner,
        node_secret_key,
    }
}

#[tokio::test]
async fn test_send() {
    lightning_test_utils::logging::setup();

    let temp_dir = tempdir().unwrap();

    // Initialize three broadcasts
    let peers = get_broadcasts(&temp_dir, 28000, 3).await;
    let query_runner = peers[0].sync_query();

    for peer in &peers {
        peer.inner.start().await;
    }

    let pub_sub1 = peers[0].broadcast().get_pubsub::<Frame>(Topic::Debug);
    let mut pub_sub2 = peers[1].broadcast().get_pubsub::<Frame>(Topic::Debug);
    let mut pub_sub3 = peers[2].broadcast().get_pubsub::<Frame>(Topic::Debug);

    // Create a message from node1
    let index = query_runner
        .pubkey_to_index(&peers[0].node_secret_key.to_pk())
        .unwrap();
    let message = Message {
        origin: index,
        signature: NodeSignature([0; 64]),
        topic: Topic::Debug,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
        payload: String::from("hello").into_bytes(),
    };

    // node2 listens to the broadcast and we make sure that it receives the same message that node1
    // sent out
    let (tx, rx2) = oneshot::channel();
    let target_message = message.clone();
    tokio::spawn(async move {
        match pub_sub2.recv().await.unwrap() {
            Frame::Message(message) => {
                assert_eq!(message, target_message);
                // Todo: old test validated signature.
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
            Frame::Message(message) => {
                assert_eq!(message, target_message);
                // Todo: old test validated signature.
                tx.send(()).unwrap();
            },
            _ => panic!("Unexpected frame"),
        }
    });

    // give time to pool to make the connections.
    // TODO(qti3e): pool should ideally have a way in future to somehow make this not needed.
    tokio::time::sleep(Duration::from_millis(300)).await;
    // node1 sends the message over the broadcast
    pub_sub1.send(&Frame::Message(message), None).await.unwrap();

    // wait until node2 and node3 received the messages before cleaning up
    rx2.await.unwrap();
    rx3.await.unwrap();

    // Clean up
    for mut peer in peers {
        peer.inner.shutdown().await;
    }
}
