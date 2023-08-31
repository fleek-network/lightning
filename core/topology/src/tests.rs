use std::collections::HashMap;

use fleek_crypto::{
    AccountOwnerSecretKey,
    ConsensusSecretKey,
    NodePublicKey,
    NodeSecretKey,
    SecretKey,
};
use lightning_application::app::Application;
use lightning_application::config::{Config as AppConfig, Mode, StorageConfig};
use lightning_application::genesis::{Genesis, GenesisLatency, GenesisNode};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodePorts;
use lightning_interfaces::{
    partial,
    ApplicationInterface,
    TopologyInterface,
    WithStartAndShutdown,
};

use crate::config::Config;
use crate::Topology;

partial!(TestBinding {
    TopologyInterface = Topology<Self>;
    ApplicationInterface = Application<Self>;
});

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
    genesis.node_info = vec![
        GenesisNode::new(
            our_owner_public_key.into(),
            our_public_key,
            "127.0.0.1".parse().unwrap(),
            our_consensus_public_key,
            "127.0.0.1".parse().unwrap(),
            our_public_key,
            NodePorts {
                primary: 48000,
                worker: 48101,
                mempool: 48102,
                rpc: 48103,
                pool: 48104,
                dht: 48105,
                handshake: 48106,
            },
            None,
            true,
        ),
        GenesisNode::new(
            node_owner_public_key1.into(),
            node_public_key1,
            "127.0.0.1".parse().unwrap(),
            node_consensus_public_key1,
            "127.0.0.1".parse().unwrap(),
            node_public_key1,
            NodePorts {
                primary: 38000,
                worker: 38101,
                mempool: 38102,
                rpc: 38103,
                pool: 38104,
                dht: 38105,
                handshake: 38106,
            },
            None,
            true,
        ),
        GenesisNode::new(
            node_owner_public_key2.into(),
            node_public_key2,
            "127.0.0.1".parse().unwrap(),
            node_consensus_public_key2,
            "127.0.0.1".parse().unwrap(),
            node_public_key2,
            NodePorts {
                primary: 38000,
                worker: 38101,
                mempool: 38102,
                rpc: 38103,
                pool: 38104,
                dht: 38105,
                handshake: 38106,
            },
            None,
            true,
        ),
    ];

    let mut latencies = Vec::new();
    let (node_lhs, node_rhs) = if our_public_key < node_public_key1 {
        (our_public_key, node_public_key1)
    } else {
        (node_public_key1, our_public_key)
    };
    latencies.push(GenesisLatency {
        node_public_key_lhs: node_lhs,
        node_public_key_rhs: node_rhs,
        latency_in_microseconds: 100000,
    });

    let (node_lhs, node_rhs) = if our_public_key < node_public_key2 {
        (our_public_key, node_public_key2)
    } else {
        (node_public_key2, our_public_key)
    };
    latencies.push(GenesisLatency {
        node_public_key_lhs: node_lhs,
        node_public_key_rhs: node_rhs,
        latency_in_microseconds: 300000,
    });

    let (node_lhs, node_rhs) = if node_public_key1 < node_public_key2 {
        (node_public_key1, node_public_key2)
    } else {
        (node_public_key2, node_public_key1)
    };
    latencies.push(GenesisLatency {
        node_public_key_lhs: node_lhs,
        node_public_key_rhs: node_rhs,
        latency_in_microseconds: 200000,
    });
    genesis.latencies = Some(latencies);

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

    let query_runner = app.sync_query();
    app.start().await;

    let topology =
        Topology::<TestBinding>::init(Config::default(), our_public_key, query_runner).unwrap();
    let (matrix, index_to_pubkey, our_index) = topology.inner.build_latency_matrix();
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
