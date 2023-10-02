use std::path::PathBuf;

use bytes::Bytes;
use fleek_crypto::{
    AccountOwnerSecretKey,
    ConsensusSecretKey,
    NodePublicKey,
    NodeSecretKey,
    SecretKey,
};
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
    ServiceScope,
    SignerInterface,
    SyncQueryRunnerInterface,
    TopologyInterface,
    WithStartAndShutdown,
};
use lightning_notifier::Notifier;
use lightning_signer::{utils, Config as SignerConfig, Signer};
use lightning_topology::{Config as TopologyConfig, Topology};

use crate::{muxer, Config, Pool};

partial!(TestBinding {
    ApplicationInterface = Application<Self>;
    PoolInterface = Pool<Self>;
    SignerInterface = Signer<Self>;
    NotifierInterface = Notifier<Self>;
    TopologyInterface = Topology<Self>;
});

struct Peer<C: Collection> {
    pool: C::PoolInterface,
    node_public_key: NodePublicKey,
}

async fn get_pools(
    num_peers: usize,
) -> (Vec<Peer<TestBinding>>, Application<TestBinding>, PathBuf) {
    let mut signers_configs = Vec::new();
    let mut genesis = Genesis::load().unwrap();
    let path = std::env::temp_dir().join("lightning-pool-test");
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();
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
                pool: 48400_u16 + i as u16,
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
        let config = Config {
            max_idle_timeout: 300,
            address: format!("0.0.0.0:{}", 48400_u16 + i as u16).parse().unwrap(),
        };
        let pool = Pool::<TestBinding, muxer::quinn::QuinnMuxer>::init(
            config,
            &signer,
            query_runner,
            notifier,
            topology,
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
    let (peers, app, path) = get_pools(2).await;
    let query_runner = app.sync_query();

    let node_index1 = query_runner
        .pubkey_to_index(peers[0].node_public_key)
        .unwrap();
    let node_index2 = query_runner
        .pubkey_to_index(peers[1].node_public_key)
        .unwrap();
    let event_handler1 = peers[0].pool.open_event(ServiceScope::Broadcast);
    let mut event_handler2 = peers[1].pool.open_event(ServiceScope::Broadcast);

    peers[0].pool.start().await;
    peers[1].pool.start().await;

    let msg = Bytes::from("hello");
    event_handler1.send_to_one(node_index2, msg.clone());
    let (sender, recv_msg) = event_handler2.receive().await.unwrap();
    assert_eq!(recv_msg, msg);
    assert_eq!(sender, node_index1);

    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
}
