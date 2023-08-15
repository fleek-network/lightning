use std::collections::HashMap;

use fleek_crypto::{
    AccountOwnerSecretKey, ConsensusSecretKey, NodePublicKey, NodeSecretKey, PublicKey, SecretKey,
};
use lightning_application::{
    app::Application,
    config::{Config as AppConfig, Mode},
    genesis::{Genesis, GenesisCommittee, GenesisLatency},
};
use lightning_interfaces::{ApplicationInterface, TopologyInterface, WithStartAndShutdown};

use crate::{config::Config, Topology};

#[tokio::test]
async fn test_build_latency_matrix() {
    let our_owner_secret_key = AccountOwnerSecretKey::generate();
    let our_owner_public_key = our_owner_secret_key.to_pk();
    let our_secret_key = NodeSecretKey::generate();
    let our_public_key = our_secret_key.to_pk();
    let our_consensus_secret_key = ConsensusSecretKey::generate();
    let our_consensus_public_key = our_consensus_secret_key.to_pk();

    let node_owner_secret_key1 = AccountOwnerSecretKey::generate();
    let node_owner_public_key1 = node_owner_secret_key1.to_pk();
    let node_secret_key1 = NodeSecretKey::generate();
    let node_public_key1 = node_secret_key1.to_pk();
    let node_consensus_secret_key1 = ConsensusSecretKey::generate();
    let node_consensus_public_key1 = node_consensus_secret_key1.to_pk();

    let node_owner_secret_key2 = AccountOwnerSecretKey::generate();
    let node_owner_public_key2 = node_owner_secret_key2.to_pk();
    let node_secret_key2 = NodeSecretKey::generate();
    let node_public_key2 = node_secret_key2.to_pk();
    let node_consensus_secret_key2 = ConsensusSecretKey::generate();
    let node_consensus_public_key2 = node_consensus_secret_key2.to_pk();

    // Init application service and store node info in application state.
    let mut genesis = Genesis::load().unwrap();
    genesis.committee = vec![
        GenesisCommittee::new(
            our_owner_public_key.to_base64(),
            our_public_key.to_base64(),
            "/ip4/127.0.0.1/udp/48000".to_owned(),
            our_consensus_public_key.to_base64(),
            "/ip4/127.0.0.1/udp/48101/http".to_owned(),
            our_public_key.to_base64(),
            "/ip4/127.0.0.1/tcp/48102/http".to_owned(),
            None,
        ),
        GenesisCommittee::new(
            node_owner_public_key1.to_base64(),
            node_public_key1.to_base64(),
            "/ip4/127.0.0.1/udp/38000".to_owned(),
            node_consensus_public_key1.to_base64(),
            "/ip4/127.0.0.1/udp/38101/http".to_owned(),
            node_public_key1.to_base64(),
            "/ip4/127.0.0.1/tcp/38102/http".to_owned(),
            None,
        ),
        GenesisCommittee::new(
            node_owner_public_key2.to_base64(),
            node_public_key2.to_base64(),
            "/ip4/127.0.0.1/udp/28000".to_owned(),
            node_consensus_public_key2.to_base64(),
            "/ip4/127.0.0.1/udp/28101/http".to_owned(),
            node_public_key2.to_base64(),
            "/ip4/127.0.0.1/tcp/28102/http".to_owned(),
            None,
        ),
    ];

    let mut latencies = Vec::new();
    let (node_lhs, node_rhs) = if our_public_key < node_public_key1 {
        (our_public_key.to_base64(), node_public_key1.to_base64())
    } else {
        (node_public_key1.to_base64(), our_public_key.to_base64())
    };
    latencies.push(GenesisLatency {
        node_public_key_lhs: node_lhs,
        node_public_key_rhs: node_rhs,
        latency_in_microseconds: 100000,
    });

    let (node_lhs, node_rhs) = if our_public_key < node_public_key2 {
        (our_public_key.to_base64(), node_public_key2.to_base64())
    } else {
        (node_public_key2.to_base64(), our_public_key.to_base64())
    };
    latencies.push(GenesisLatency {
        node_public_key_lhs: node_lhs,
        node_public_key_rhs: node_rhs,
        latency_in_microseconds: 300000,
    });

    let (node_lhs, node_rhs) = if node_public_key1 < node_public_key2 {
        (node_public_key1.to_base64(), node_public_key2.to_base64())
    } else {
        (node_public_key2.to_base64(), node_public_key1.to_base64())
    };
    latencies.push(GenesisLatency {
        node_public_key_lhs: node_lhs,
        node_public_key_rhs: node_rhs,
        latency_in_microseconds: 200000,
    });
    genesis.latencies = Some(latencies);

    let app = Application::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .unwrap();

    let query_runner = app.sync_query();
    app.start().await;

    let topology = Topology::init(Config::default(), our_public_key, query_runner).unwrap();
    let (matrix, index_to_pubkey, our_index) = topology.build_latency_matrix();
    let pubkey_to_index: HashMap<NodePublicKey, usize> = index_to_pubkey
        .iter()
        .map(|(index, pubkey)| (*pubkey, *index))
        .collect();

    assert_eq!(matrix.shape()[0], matrix.shape()[1]);
    assert_eq!(matrix.shape()[0], index_to_pubkey.len());
    assert_eq!(
        our_index.unwrap(),
        *pubkey_to_index.get(&our_public_key).unwrap()
    );

    let our_index = *pubkey_to_index.get(&our_public_key).unwrap();
    let index1 = *pubkey_to_index.get(&node_public_key1).unwrap();
    let index2 = *pubkey_to_index.get(&node_public_key2).unwrap();
    assert_eq!(matrix[[our_index, index1]], 100000);
    assert_eq!(matrix[[our_index, index2]], 300000);
    assert_eq!(matrix[[index1, index2]], 200000);
}
