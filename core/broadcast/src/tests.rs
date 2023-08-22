use std::time::SystemTime;

use anyhow::Result;
use fleek_crypto::{AccountOwnerSecretKey, NodePublicKey, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::Mode;
use lightning_application::genesis::{Genesis, GenesisLatency, GenesisNode};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::schema::broadcast::BroadcastFrame;
use lightning_interfaces::schema::AutoImplSerde;
use lightning_interfaces::types::{NodePorts, ServiceScope, Staking, Topic};
use lightning_interfaces::{
    partial,
    ApplicationInterface,
    BroadcastInterface,
    ConnectionPoolInterface,
    NotifierInterface,
    PubSub,
    SignerInterface,
    TopologyInterface,
    WithStartAndShutdown,
};
use lightning_notifier::Notifier;
use lightning_signer::Signer;
use lightning_topology::Topology;
use mock::pool::transport::GlobalMemoryTransport;
use mock::pool::{Config, ConnectionPool};
use serde::{Deserialize, Serialize};

use crate::{config, Broadcast};

partial!(TestBinding {
    ApplicationInterface = Application<Self>;
    SignerInterface = Signer<Self>;
    ConnectionPoolInterface = ConnectionPool<Self>;
    TopologyInterface = Topology<Self>;
    NotifierInterface = Notifier<Self>;
});

/// Mock pubsub topic message
#[derive(Clone, Serialize, Deserialize)]
struct DebugMessage(usize);
impl AutoImplSerde for DebugMessage {}

#[tokio::test]
async fn pubsub_send_recv() -> Result<()> {
    let global_transport = GlobalMemoryTransport::default();

    let signer_config_a = lightning_signer::Config::test();
    let (consensus_key_a, node_key_a) = signer_config_a.load_test_keys();

    let signer_config_b = lightning_signer::Config::test2();
    let (consensus_key_b, node_key_b) = signer_config_b.load_test_keys();

    let consensus_key_a = consensus_key_a.to_pk();
    let consensus_key_b = consensus_key_b.to_pk();
    let node_key_a = node_key_a.to_pk();
    let node_key_b = node_key_b.to_pk();

    // Setup genesis
    let mut genesis = Genesis::load()?;
    genesis.node_info = vec![
        GenesisNode::new(
            AccountOwnerSecretKey::generate().to_pk().into(),
            node_key_a,
            "127.0.0.1".parse().unwrap(),
            consensus_key_a,
            "127.0.0.1".parse().unwrap(),
            node_key_a,
            NodePorts {
                primary: 48100,
                worker: 48101,
                mempool: 48102,
                rpc: 48103,
                pool: 48104,
                dht: 48105,
                handshake: 48106,
            },
            Some(Staking {
                staked: 10000_u32.into(),
                ..Default::default()
            }),
            true,
        ),
        GenesisNode::new(
            AccountOwnerSecretKey::generate().to_pk().into(),
            node_key_b,
            "127.0.0.1".parse().unwrap(),
            consensus_key_b,
            "127.0.0.1".parse().unwrap(),
            node_key_b,
            NodePorts {
                primary: 48100,
                worker: 48101,
                mempool: 48102,
                rpc: 48103,
                pool: 48104,
                dht: 48105,
                handshake: 48106,
            },
            Some(Staking {
                staked: 10000_u32.into(),
                ..Default::default()
            }),
            true,
        ),
    ];
    genesis.latencies = Some(vec![GenesisLatency {
        node_public_key_lhs: node_key_a,
        node_public_key_rhs: node_key_b,
        latency_in_microseconds: 42000,
    }]);
    genesis.epoch_start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_millis() as u64;

    // Setup shared application state (it will never get modified)
    let application = lightning_application::app::Application::<TestBinding>::init(
        lightning_application::config::Config {
            mode: Mode::Test,
            genesis: Some(genesis),
        },
    )?;
    let query_runner = application.sync_query();

    // setup signers
    let signer_a = Signer::<TestBinding>::init(signer_config_a, query_runner.clone())?;
    let signer_b = Signer::<TestBinding>::init(signer_config_b, query_runner.clone())?;

    let topology = Topology::<TestBinding>::init(
        lightning_topology::config::Config::default(),
        // use a dummy key to force topology to always return all nodes
        NodePublicKey([0u8; 32]),
        query_runner.clone(),
    )?;

    // Node A
    let mut pool_a =
        ConnectionPool::<TestBinding>::init(Config {}, &signer_a, query_runner.clone());
    pool_a.with_transport(global_transport.clone());
    pool_a.start().await;
    let listener_connector = pool_a.bind::<BroadcastFrame>(ServiceScope::Broadcast);
    let notifier = Notifier::<TestBinding>::init(&application);
    let broadcast_a = Broadcast::<TestBinding>::init(
        config::Config::default(),
        listener_connector,
        topology.clone(),
        &signer_a,
        notifier,
    )?;
    let pubsub_a = broadcast_a.get_pubsub::<DebugMessage>(Topic::Debug);

    // Node B
    let mut pool_b = ConnectionPool::init(Config {}, &signer_b, query_runner.clone());
    pool_b.with_transport(global_transport.clone());
    pool_b.start().await;
    let listener_connector = pool_b.bind::<BroadcastFrame>(ServiceScope::Broadcast);
    let notifier = Notifier::init(&application);
    let broadcast_b = Broadcast::<TestBinding>::init(
        config::Config::default(),
        listener_connector,
        topology.clone(),
        &signer_b,
        notifier,
    )?;
    let mut pubsub_b = broadcast_b.get_pubsub::<DebugMessage>(Topic::Debug);

    broadcast_a.start().await;
    broadcast_b.start().await;

    pubsub_a.send(&DebugMessage(420)).await;

    let message = pubsub_b
        .recv()
        .await
        .expect("failed to get message via pubsub");
    assert_eq!(message.0, 420);

    Ok(())
}
