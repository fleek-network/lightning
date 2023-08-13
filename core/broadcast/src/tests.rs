use std::{sync::Arc, time::SystemTime};

use anyhow::Result;
use fleek_crypto::{AccountOwnerSecretKey, NodePublicKey, PublicKey, SecretKey};
use lightning_application::{
    config::Mode,
    genesis::{Genesis, GenesisCommittee, GenesisLatency},
    query_runner::QueryRunner,
};
use lightning_interfaces::{
    schema::AutoImplSerde, ApplicationInterface, BroadcastInterface, ConnectionPoolInterface,
    NotifierInterface, PubSub, ServiceScope, SignerInterface, Topic, TopologyInterface,
    WithStartAndShutdown,
};
use lightning_notifier::Notifier;
use lightning_signer::Signer;
use lightning_topology::Topology;
use mock::pool::{transport::GlobalMemoryTransport, Config, ConnectionPool};
use serde::{Deserialize, Serialize};

use crate::{schema::BroadcastFrame, Broadcast};

/// Mock pubsub topic message
#[derive(Clone, Serialize, Deserialize)]
struct DebugMessage(usize);
impl AutoImplSerde for DebugMessage {}

#[tokio::test]
async fn pubsub_send_recv() -> Result<()> {
    let global_transport = GlobalMemoryTransport::default();

    let signer_config_a = lightning_signer::Config::test();
    let (key_a, networking_key_a) = signer_config_a.load_test_keys();
    let (key_a, networking_key_a) = (
        key_a.to_pk().to_base64(),
        networking_key_a.to_pk().to_base64(),
    );
    let signer_config_b = lightning_signer::Config::test2();
    let (key_b, networking_key_b) = signer_config_b.load_test_keys();
    let (key_b, networking_key_b) = (
        key_b.to_pk().to_base64(),
        networking_key_b.to_pk().to_base64(),
    );

    // Setup genesis
    let mut genesis = Genesis::load()?;
    genesis.committee = vec![
        GenesisCommittee::new(
            AccountOwnerSecretKey::generate().to_pk().to_base64(),
            key_a.clone(),
            "/ip4/127.0.0.1/udp/48100".to_owned(),
            networking_key_a.clone(),
            "/ip4/127.0.0.1/udp/48101/http".to_owned(),
            networking_key_a,
            "/ip4/127.0.0.1/tcp/48102/http".to_owned(),
            Some(10000),
        ),
        GenesisCommittee::new(
            AccountOwnerSecretKey::generate().to_pk().to_base64(),
            key_b.clone(),
            "/ip4/127.0.0.1/udp/48100".to_owned(),
            networking_key_b.clone(),
            "/ip4/127.0.0.1/udp/48101/http".to_owned(),
            networking_key_b,
            "/ip4/127.0.0.1/tcp/48102/http".to_owned(),
            Some(10000),
        ),
    ];
    genesis.latencies = Some(vec![GenesisLatency {
        node_public_key_lhs: key_a,
        node_public_key_rhs: key_b,
        latency_in_microseconds: 42000,
    }]);
    genesis.epoch_start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_millis() as u64;

    // Setup shared application state (it will never get modified)
    let application =
        lightning_application::app::Application::init(lightning_application::config::Config {
            mode: Mode::Test,
            genesis: Some(genesis),
        })?;
    let query_runner = application.sync_query();

    // setup signers
    let signer_a = Signer::init(signer_config_a, query_runner.clone())?;
    let signer_b = Signer::init(signer_config_b, query_runner.clone())?;

    let topology = Arc::new(Topology::init(
        lightning_topology::config::Config::default(),
        // use a dummy key to force topology to always return all nodes
        NodePublicKey([0u8; 96]),
        query_runner.clone(),
    )?);

    // Node A
    let mut pool_a = ConnectionPool::init(Config {}, &signer_a, query_runner.clone());
    pool_a.with_transport(global_transport.clone());
    pool_a.start().await;
    let listener_connector = pool_a.bind::<BroadcastFrame>(ServiceScope::Broadcast);
    let notifier = Notifier::init(query_runner.clone());
    let broadcast_a = Broadcast::<ConnectionPool<Signer, QueryRunner>, _, _, _>::init(
        crate::config::Config {},
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
    let notifier = Notifier::init(query_runner.clone());
    let broadcast_b = Broadcast::<ConnectionPool<Signer, QueryRunner>, _, _, _>::init(
        crate::config::Config {},
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
