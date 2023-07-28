use std::collections::HashMap;

use fleek_crypto::{NodePublicKey, NodeSecretKey, PublicKey, SecretKey};
use freek_application::{
    app::Application,
    config::{Config as AppConfig, Mode},
    genesis::{Genesis, GenesisLatency},
};
use freek_interfaces::{ApplicationInterface, TopologyInterface, WithStartAndShutdown};

use crate::{config::Config, Topology};

#[tokio::test]
async fn test_build_latency_matrix() {
    let our_secret_key = NodeSecretKey::generate();
    let our_public_key = our_secret_key.to_pk();

    let node_secret_key1 = NodeSecretKey::generate();
    let node_public_key1 = node_secret_key1.to_pk();

    let node_secret_key2 = NodeSecretKey::generate();
    let node_public_key2 = node_secret_key2.to_pk();

    // Init application service and store node info in application state.
    let mut genesis = Genesis::load().unwrap();

    genesis.latencies.push(GenesisLatency {
        node_public_key_lhs: our_public_key.to_base64(),
        node_public_key_rhs: node_public_key1.to_base64(),
        latency_in_microseconds: 100000,
    });

    genesis.latencies.push(GenesisLatency {
        node_public_key_lhs: our_public_key.to_base64(),
        node_public_key_rhs: node_public_key2.to_base64(),
        latency_in_microseconds: 300000,
    });

    genesis.latencies.push(GenesisLatency {
        node_public_key_lhs: node_public_key1.to_base64(),
        node_public_key_rhs: node_public_key2.to_base64(),
        latency_in_microseconds: 200000,
    });

    genesis.latencies.push(GenesisLatency {
        node_public_key_lhs: node_public_key2.to_base64(),
        node_public_key_rhs: node_public_key1.to_base64(),
        latency_in_microseconds: 300000,
    });

    let app = Application::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .await
    .unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    let topology = Topology::init(Config::default(), our_public_key, query_runner)
        .await
        .unwrap();
    let (matrix, index_to_pubkey, our_index) = topology.build_latency_matrix();
    let pubkey_to_index: HashMap<NodePublicKey, usize> = index_to_pubkey
        .iter()
        .map(|(index, pubkey)| (*pubkey, *index))
        .collect();

    assert_eq!(
        our_index.unwrap(),
        *pubkey_to_index.get(&our_public_key).unwrap()
    );

    let our_index = *pubkey_to_index.get(&our_public_key).unwrap();
    let index1 = *pubkey_to_index.get(&node_public_key1).unwrap();
    let index2 = *pubkey_to_index.get(&node_public_key2).unwrap();
    assert_eq!(matrix[[our_index, index1]], 100000);
    assert_eq!(matrix[[our_index, index2]], 300000);
    assert_eq!(matrix[[index1, index2]], 250000);
}
