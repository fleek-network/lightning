use std::collections::{BTreeSet, HashMap};

use fleek_crypto::{
    AccountOwnerSecretKey,
    ConsensusSecretKey,
    NodePublicKey,
    NodeSecretKey,
    SecretKey,
};
use lightning_application::app::Application;
use lightning_application::config::ApplicationConfig;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{Genesis, GenesisLatency, GenesisNode, NodePorts, Participation};
use lightning_node::Node;
use lightning_notifier::Notifier;
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_utils::application::QueryRunnerExt;
use tempfile::tempdir;

use crate::core::build_latency_matrix;
use crate::Topology;

partial_node_components!(TestBinding {
    ConfigProviderInterface = JsonConfigProvider;
    TopologyInterface = Topology<Self>;
    ApplicationInterface = Application<Self>;
    NotifierInterface = Notifier<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
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
    let mut genesis = Genesis {
        node_info: vec![
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
                    pinger: 48106,
                    handshake: Default::default(),
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
                    pinger: 38106,
                    handshake: Default::default(),
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
                    pinger: 38106,
                    handshake: Default::default(),
                },
                None,
                true,
            ),
        ],

        ..Default::default()
    };

    let mut latencies = Vec::new();
    let (node_lhs, node_rhs) = if our_public_key < node_public_key1 {
        (our_public_key, node_public_key1)
    } else {
        (node_public_key1, our_public_key)
    };
    latencies.push(GenesisLatency {
        node_public_key_lhs: node_lhs,
        node_public_key_rhs: node_rhs,
        latency_in_millis: 1000,
    });

    let (node_lhs, node_rhs) = if our_public_key < node_public_key2 {
        (our_public_key, node_public_key2)
    } else {
        (node_public_key2, our_public_key)
    };
    latencies.push(GenesisLatency {
        node_public_key_lhs: node_lhs,
        node_public_key_rhs: node_rhs,
        latency_in_millis: 3000,
    });

    let (node_lhs, node_rhs) = if node_public_key1 < node_public_key2 {
        (node_public_key1, node_public_key2)
    } else {
        (node_public_key2, node_public_key1)
    };
    latencies.push(GenesisLatency {
        node_public_key_lhs: node_lhs,
        node_public_key_rhs: node_rhs,
        latency_in_millis: 2000,
    });
    genesis.latencies = Some(latencies);

    let temp_dir = tempdir().unwrap();
    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let mut node = Node::<TestBinding>::init_with_provider(
        fdi::Provider::default().with(
            JsonConfigProvider::default()
                .with::<Application<TestBinding>>(ApplicationConfig::test(genesis_path)),
        ),
    )
    .expect("failed to init node");
    node.start().await;

    let query_runner = node
        .provider
        .get::<c!(TestBinding::ApplicationInterface::SyncExecutor)>();
    let latencies = query_runner.get_current_latencies();
    let valid_pubkeys: BTreeSet<NodePublicKey> = query_runner
        .get_node_registry(None)
        .into_iter()
        .filter(|node_info| node_info.info.participation == Participation::True)
        .map(|node_info| node_info.info.public_key)
        .collect();

    let (matrix, index_to_pubkey, our_index) =
        build_latency_matrix(our_public_key, latencies, valid_pubkeys);
    //let (matrix, index_to_pubkey, our_index) = topology.inner.build_latency_matrix();
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
    assert_eq!(matrix[[our_index, index1]], 1000);
    assert_eq!(matrix[[our_index, index2]], 3000);
    assert_eq!(matrix[[index1, index2]], 2000);

    node.shutdown().await;
}

#[tokio::test]
async fn test_receive_connections() {
    let temp_dir = tempdir().unwrap();
    let genesis_path = Genesis::default()
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();
    let mut node = Node::<TestBinding>::init_with_provider(
        fdi::Provider::default().with(
            JsonConfigProvider::default()
                .with::<Application<TestBinding>>(ApplicationConfig::test(genesis_path)),
        ),
    )
    .unwrap();

    let mut topology_rx = node.provider.get::<Topology<TestBinding>>().get_receiver();

    let connections = topology_rx.borrow_and_update().clone();
    // The topology sends an empty vec in its init function because the tokio watch channel has to
    // be initialized with a value.
    assert!(connections.is_empty());

    node.start().await;

    // Once the topology starts, it will compute the actual connections and send them.
    topology_rx.changed().await.unwrap();
    let connections = topology_rx.borrow_and_update().clone();
    assert!(!connections.is_empty());

    node.shutdown().await;
}
