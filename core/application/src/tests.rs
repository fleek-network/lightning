use std::collections::BTreeMap;

use affair::Socket;
use anyhow::{anyhow, Result};
use fleek_crypto::{AccountOwnerSecretKey, ConsensusSecretKey, NodeSecretKey, SecretKey};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{
    Block,
    BlockExecutionResponse,
    ExecutionData,
    ExecutionError,
    HandshakePorts,
    NodePorts,
    Participation,
    ReputationMeasurements,
    TransactionRequest,
    TransactionResponse,
    UpdateMethod,
    UpdatePayload,
    UpdateRequest,
};
use lightning_interfaces::{
    partial,
    ApplicationInterface,
    ExecutionEngineSocket,
    SyncQueryRunnerInterface,
    ToDigest,
};
use lightning_test_utils::{random, reputation};

use crate::app::Application;
use crate::config::{Config, Mode, StorageConfig};
use crate::genesis::{Genesis, GenesisNode};
use crate::query_runner::QueryRunner;

partial!(TestBinding {
    ApplicationInterface = Application<Self>;
});

// This is a helper struct for keeping track of a node's private keys.
// Many tests require us to submit transactions.
#[derive(Clone)]
struct GenesisCommitteeKeystore {
    _owner_secret_key: AccountOwnerSecretKey,
    node_secret_key: NodeSecretKey,
    _consensus_secret_key: ConsensusSecretKey,
    _worker_secret_key: NodeSecretKey,
}

// Init the app and return the execution engine socket that would go to narwhal and the query socket
// that could go to anyone
fn init_app(config: Option<Config>) -> (ExecutionEngineSocket, QueryRunner) {
    let config = config.or(Some(Config {
        genesis: None,
        mode: Mode::Dev,
        testnet: false,
        storage: StorageConfig::InMemory,
        db_path: None,
        db_options: None,
    }));
    let app = Application::<TestBinding>::init(config.unwrap(), Default::default()).unwrap();

    (app.transaction_executor(), app.sync_query())
}

// Helper function to create a genesis committee.
// This is useful for tests where we need to seed the application state with nodes.
fn create_genesis_committee(
    num_members: usize,
) -> (Vec<GenesisNode>, Vec<GenesisCommitteeKeystore>) {
    let mut keystore = Vec::new();
    let mut committee = Vec::new();
    (0..num_members as u16).for_each(|i| {
        let node_secret_key = NodeSecretKey::generate();
        let consensus_secret_key = ConsensusSecretKey::generate();
        let owner_secret_key = AccountOwnerSecretKey::generate();
        let node = create_committee_member(
            &owner_secret_key,
            &node_secret_key,
            &consensus_secret_key,
            i,
        );
        committee.push(node);
        keystore.push(GenesisCommitteeKeystore {
            _owner_secret_key: owner_secret_key,
            _worker_secret_key: node_secret_key.clone(),
            node_secret_key,
            _consensus_secret_key: consensus_secret_key,
        });
    });
    (committee, keystore)
}

fn create_committee_member(
    owner_secret_key: &AccountOwnerSecretKey,
    node_secret_key: &NodeSecretKey,
    consensus_secret_key: &ConsensusSecretKey,
    index: u16,
) -> GenesisNode {
    let node_public_key = node_secret_key.to_pk();
    let consensus_public_key = consensus_secret_key.to_pk();
    let owner_public_key = owner_secret_key.to_pk();
    GenesisNode::new(
        owner_public_key.into(),
        node_public_key,
        "127.0.0.1".parse().unwrap(),
        consensus_public_key,
        "127.0.0.1".parse().unwrap(),
        node_public_key,
        NodePorts {
            primary: 8000 + index,
            worker: 9000 + index,
            mempool: 7000 + index,
            rpc: 6000 + index,
            pool: 5000 + index,
            dht: 4000 + index,
            pinger: 2000 + index,
            handshake: HandshakePorts {
                http: 5000 + index,
                webrtc: 6000 + index,
                webtransport: 7000 + index,
            },
        },
        None,
        true,
    )
}

// Helper function to create an update request from a update method.
fn get_update_request_node(
    method: UpdateMethod,
    secret_key: &NodeSecretKey,
    nonce: u64,
) -> UpdateRequest {
    let payload = UpdatePayload {
        sender: secret_key.to_pk().into(),
        nonce,
        method,
    };
    let digest = payload.to_digest();
    let signature = secret_key.sign(&digest);
    UpdateRequest {
        signature: signature.into(),
        payload,
    }
}

// Helper function that submits a transaction to the application.
async fn run_transaction(
    requests: Vec<TransactionRequest>,
    update_socket: &Socket<Block, BlockExecutionResponse>,
) -> Result<BlockExecutionResponse> {
    let res = update_socket
        .run(Block {
            transactions: requests,
            digest: [0; 32],
        })
        .await
        .map_err(|r| anyhow!(format!("{r:?}")))?;
    Ok(res)
}

//////////////////////////////////////////////////////////////////////////////////
////////////////// This is where the actual tests are defined ////////////////////
//////////////////////////////////////////////////////////////////////////////////

#[tokio::test]
async fn test_epoch_change() {
    // Create a genesis committee and seed the application state with it.
    let (committee, keystore) = create_genesis_committee(4);
    let mut genesis = Genesis::load().unwrap();
    let committee_size = committee.len();
    genesis.node_info = committee;
    let (update_socket, query_runner) = init_app(Some(Config {
        genesis: Some(genesis),
        mode: Mode::Test,
        testnet: false,
        storage: StorageConfig::InMemory,
        db_path: None,
        db_options: None,
    }));

    let required_signals = 2 * committee_size / 3 + 1;

    // Have (required_signals - 1) say they are ready to change epoch
    // make sure the epoch doesnt change each time someone signals
    for node in keystore.iter().take(required_signals - 1) {
        let req = get_update_request_node(
            UpdateMethod::ChangeEpoch { epoch: 0 },
            &node.node_secret_key,
            1,
        );

        let res = run_transaction(vec![req.into()], &update_socket)
            .await
            .unwrap();
        // Make sure epoch didnt change
        assert!(!res.change_epoch);
    }
    // check that the current epoch is still 0
    assert_eq!(query_runner.get_epoch_info().epoch, 0);

    // Have the last needed committee member signal the epoch change and make sure it changes
    let req = get_update_request_node(
        UpdateMethod::ChangeEpoch { epoch: 0 },
        &keystore[required_signals].node_secret_key,
        1,
    );
    let res = run_transaction(vec![req.into()], &update_socket)
        .await
        .unwrap();
    assert!(res.change_epoch);

    // Query epoch info and make sure it incremented to new epoch
    assert_eq!(query_runner.get_epoch_info().epoch, 1);
}

#[tokio::test]
async fn test_submit_rep_measurements() {
    let (committee, keystore) = create_genesis_committee(4);
    let mut genesis = Genesis::load().unwrap();
    genesis.node_info = committee;
    let (update_socket, query_runner) = init_app(Some(Config {
        genesis: Some(genesis),
        mode: Mode::Test,
        testnet: false,
        storage: StorageConfig::InMemory,
        db_path: None,
        db_options: None,
    }));

    let mut map = BTreeMap::new();
    let mut rng = random::get_seedable_rng();

    let measurements1 = reputation::generate_reputation_measurements(&mut rng, 0.1);
    let peer1 = keystore[1].node_secret_key.to_pk();
    let peer_index1 = query_runner.pubkey_to_index(peer1).unwrap();
    map.insert(peer_index1, measurements1.clone());

    let measurements2 = reputation::generate_reputation_measurements(&mut rng, 0.1);
    let peer2 = keystore[2].node_secret_key.to_pk();
    let peer_index2 = query_runner.pubkey_to_index(peer2).unwrap();
    map.insert(peer_index2, measurements2.clone());

    let reporting_node_key = keystore[0].node_secret_key.to_pk();
    let reporting_node_index = query_runner.pubkey_to_index(reporting_node_key).unwrap();
    let req = get_update_request_node(
        UpdateMethod::SubmitReputationMeasurements { measurements: map },
        &keystore[0].node_secret_key,
        1,
    );
    if let Err(e) = run_transaction(vec![req.into()], &update_socket).await {
        panic!("{e}");
    }

    let rep_measurements1 = query_runner.get_rep_measurements(&peer_index1);
    assert_eq!(rep_measurements1.len(), 1);
    assert_eq!(rep_measurements1[0].reporting_node, reporting_node_index);
    assert_eq!(rep_measurements1[0].measurements, measurements1);

    let rep_measurements2 = query_runner.get_rep_measurements(&peer_index2);
    assert_eq!(rep_measurements2.len(), 1);
    assert_eq!(rep_measurements2[0].reporting_node, reporting_node_index);
    assert_eq!(rep_measurements2[0].measurements, measurements2);
}

#[tokio::test]
async fn test_submit_rep_measurements_twice() {
    let (committee, keystore) = create_genesis_committee(4);
    let mut genesis = Genesis::load().unwrap();
    genesis.node_info = committee;
    let (update_socket, query_runner) = init_app(Some(Config {
        genesis: Some(genesis),
        mode: Mode::Test,
        testnet: false,
        storage: StorageConfig::InMemory,
        db_path: None,
        db_options: None,
    }));

    let mut map = BTreeMap::new();
    let mut rng = random::get_seedable_rng();

    let measurements = reputation::generate_reputation_measurements(&mut rng, 0.1);
    let peer = keystore[1].node_secret_key.to_pk();
    let peer_index = query_runner.pubkey_to_index(peer).unwrap();
    map.insert(peer_index, measurements.clone());

    // Submit the reputation measurements
    let req = get_update_request_node(
        UpdateMethod::SubmitReputationMeasurements {
            measurements: map.clone(),
        },
        &keystore[0].node_secret_key,
        1,
    );
    match run_transaction(vec![req.into()], &update_socket).await {
        Ok(response) => {
            assert_eq!(
                response.txn_receipts[0].response,
                TransactionResponse::Success(ExecutionData::None)
            );
        },
        Err(e) => panic!("{e}"),
    }

    // Attempt to submit reputation measurements twice per epoch.
    // This transaction should revert because each node only can submit its reputation measurements
    // once per epoch.
    let req = get_update_request_node(
        UpdateMethod::SubmitReputationMeasurements { measurements: map },
        &keystore[0].node_secret_key,
        2,
    );
    match run_transaction(vec![req.into()], &update_socket).await {
        Ok(response) => {
            assert_eq!(
                response.txn_receipts[0].response,
                TransactionResponse::Revert(ExecutionError::AlreadySubmittedMeasurements)
            );
        },
        Err(e) => panic!("{e}"),
    }
}

#[tokio::test]
async fn test_rep_scores() {
    let (committee, keystore) = create_genesis_committee(4);
    let committee_len = committee.len();
    let mut genesis = Genesis::load().unwrap();
    genesis.node_info = committee;
    let (update_socket, query_runner) = init_app(Some(Config {
        genesis: Some(genesis),
        mode: Mode::Test,
        testnet: false,
        storage: StorageConfig::InMemory,
        db_path: None,
        db_options: None,
    }));
    let required_signals = 2 * committee_len / 3 + 1;

    let mut rng = random::get_seedable_rng();

    let mut map = BTreeMap::new();
    let measurements = reputation::generate_reputation_measurements(&mut rng, 0.1);
    let peer1 = keystore[2].node_secret_key.to_pk();
    let peer_index1 = query_runner.pubkey_to_index(peer1).unwrap();
    map.insert(peer_index1, measurements.clone());

    let measurements = reputation::generate_reputation_measurements(&mut rng, 0.1);
    let peer2 = keystore[3].node_secret_key.to_pk();
    let peer_index2 = query_runner.pubkey_to_index(peer2).unwrap();
    map.insert(peer_index2, measurements.clone());

    let req = get_update_request_node(
        UpdateMethod::SubmitReputationMeasurements { measurements: map },
        &keystore[0].node_secret_key,
        1,
    );

    if let Err(e) = run_transaction(vec![req.into()], &update_socket).await {
        panic!("{e}");
    }

    let mut map = BTreeMap::new();
    let measurements = reputation::generate_reputation_measurements(&mut rng, 0.1);
    map.insert(peer_index1, measurements.clone());

    let measurements = reputation::generate_reputation_measurements(&mut rng, 0.1);
    map.insert(peer_index2, measurements.clone());

    let req = get_update_request_node(
        UpdateMethod::SubmitReputationMeasurements { measurements: map },
        &keystore[1].node_secret_key,
        1,
    );

    if let Err(e) = run_transaction(vec![req.into()], &update_socket).await {
        panic!("{e}");
    }

    // Change epoch so that rep scores will be calculated from the measurements.
    for (i, node) in keystore.iter().enumerate().take(required_signals) {
        // Not the prettiest solution but we have to keep track of the nonces somehow.
        let nonce = if i == 0 || i == 1 { 2 } else { 1 };
        let req = get_update_request_node(
            UpdateMethod::ChangeEpoch { epoch: 0 },
            &node.node_secret_key,
            nonce,
        );
        run_transaction(vec![req.into()], &update_socket)
            .await
            .unwrap();
    }

    assert!(query_runner.get_reputation(&peer_index1).is_some());
    assert!(query_runner.get_reputation(&peer_index2).is_some());
}

#[tokio::test]
async fn test_uptime_participation() {
    let (mut committee, keystore) = create_genesis_committee(4);
    let committee_len = committee.len();
    let mut genesis = Genesis::load().unwrap();
    committee[0].reputation = Some(40);
    committee[1].reputation = Some(80);
    genesis.node_info = committee;
    let (update_socket, query_runner) = init_app(Some(Config {
        genesis: Some(genesis),
        mode: Mode::Test,
        testnet: false,
        storage: StorageConfig::InMemory,
        db_path: None,
        db_options: None,
    }));
    let required_signals = 2 * committee_len / 3 + 1;

    let mut map = BTreeMap::new();
    let measurements = ReputationMeasurements {
        latency: None,
        interactions: None,
        inbound_bandwidth: None,
        outbound_bandwidth: None,
        bytes_received: None,
        bytes_sent: None,
        uptime: Some(5),
        hops: None,
    };
    let peer1 = keystore[2].node_secret_key.to_pk();
    let peer_index1 = query_runner.pubkey_to_index(peer1).unwrap();
    map.insert(peer_index1, measurements.clone());
    let measurements = ReputationMeasurements {
        latency: None,
        interactions: None,
        inbound_bandwidth: None,
        outbound_bandwidth: None,
        bytes_received: None,
        bytes_sent: None,
        uptime: Some(20),
        hops: None,
    };
    let peer2 = keystore[3].node_secret_key.to_pk();
    let peer_index2 = query_runner.pubkey_to_index(peer2).unwrap();
    map.insert(peer_index2, measurements.clone());

    let req = get_update_request_node(
        UpdateMethod::SubmitReputationMeasurements { measurements: map },
        &keystore[0].node_secret_key,
        1,
    );

    if let Err(e) = run_transaction(vec![req.into()], &update_socket).await {
        panic!("{e}");
    }

    let mut map = BTreeMap::new();
    let measurements = ReputationMeasurements {
        latency: None,
        interactions: None,
        inbound_bandwidth: None,
        outbound_bandwidth: None,
        bytes_received: None,
        bytes_sent: None,
        uptime: Some(9),
        hops: None,
    };
    map.insert(peer_index1, measurements.clone());

    let measurements = ReputationMeasurements {
        latency: None,
        interactions: None,
        inbound_bandwidth: None,
        outbound_bandwidth: None,
        bytes_received: None,
        bytes_sent: None,
        uptime: Some(25),
        hops: None,
    };
    map.insert(peer_index2, measurements.clone());

    let req = get_update_request_node(
        UpdateMethod::SubmitReputationMeasurements { measurements: map },
        &keystore[1].node_secret_key,
        1,
    );

    if let Err(e) = run_transaction(vec![req.into()], &update_socket).await {
        panic!("{e}");
    }

    // Change epoch so that rep scores will be calculated from the measurements.
    for (i, node) in keystore.iter().enumerate().take(required_signals) {
        // Not the prettiest solution but we have to keep track of the nonces somehow.
        let nonce = if i == 0 || i == 1 { 2 } else { 1 };
        let req = get_update_request_node(
            UpdateMethod::ChangeEpoch { epoch: 0 },
            &node.node_secret_key,
            nonce,
        );
        run_transaction(vec![req.into()], &update_socket)
            .await
            .unwrap();
    }

    let node_info1 = query_runner.get_node_info(&peer1).unwrap();
    let node_info2 = query_runner.get_node_info(&peer2).unwrap();

    assert_eq!(node_info1.participation, Participation::False);
    assert_eq!(node_info2.participation, Participation::True);
}
