use std::collections::BTreeMap;
use std::net::IpAddr;
use std::time::SystemTime;

use affair::Socket;
use anyhow::{anyhow, Result};
use fleek_crypto::{
    AccountOwnerSecretKey,
    ConsensusPublicKey,
    ConsensusSecretKey,
    EthAddress,
    NodePublicKey,
    NodeSecretKey,
    SecretKey,
};
use hp_fixed::signed::HpFixed;
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{
    Block,
    BlockExecutionResponse,
    DeliveryAcknowledgment,
    ExecutionData,
    ExecutionError,
    HandshakePorts,
    NodePorts,
    Participation,
    ProofOfConsensus,
    ProtocolParams,
    ReputationMeasurements,
    Tokens,
    TotalServed,
    TransactionRequest,
    TransactionResponse,
    UpdateMethod,
    UpdatePayload,
    UpdateRequest,
    MAX_MEASUREMENTS_PER_TX,
    MAX_MEASUREMENTS_SUBMIT,
};
use lightning_interfaces::{
    partial,
    ApplicationInterface,
    ExecutionEngineSocket,
    PagingParams,
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

pub struct Params {
    epoch_time: Option<u64>,
    max_inflation: Option<u16>,
    protocol_share: Option<u16>,
    node_share: Option<u16>,
    service_builder_share: Option<u16>,
    max_boost: Option<u16>,
    supply_at_genesis: Option<u64>,
}

/// Helper struct for keeping track of a node's private keys.
#[derive(Clone)]
struct GenesisCommitteeKeystore {
    _owner_secret_key: AccountOwnerSecretKey,
    node_secret_key: NodeSecretKey,
    consensus_secret_key: ConsensusSecretKey,
    _worker_secret_key: NodeSecretKey,
}

/// Helper macro executing single Update within a single Block.
/// Asserts that submission occurred.
/// Transaction Result may be Success or Revert - `TransactionResponse`.
///
///  # Arguments
///
/// * `update: UpdateRequest` - The update request to be executed.
/// * `socket: &ExecutionEngineSocket` - Socket for submitting transaction.
///
/// # Returns
///
/// * `BlockExecutionResponse`
macro_rules! run_update {
    ($update:expr,$socket:expr) => {{
        let updates = vec![$update.into()];
        run_transactions!(updates, $socket)
    }};
}

/// Helper macro executing many Updates within a single Block.
/// Asserts that submission occurred.
/// Transaction Result may be Success or Revert - `TransactionResponse`.
///
///  # Arguments
///
/// * `updates: Vec<UpdateRequest>` - Vector of update requests to be executed.
/// * `socket: &ExecutionEngineSocket` - Socket for submitting transaction.
///
/// # Returns
///
/// * `BlockExecutionResponse`
macro_rules! run_updates {
    ($updates:expr,$socket:expr) => {{
        let txs = $updates.into_iter().map(|update| update.into()).collect();
        run_transactions!(txs, $socket)
    }};
}

/// Helper macro executing many Transactions within a single Block.
/// Asserts that submission occurred.
/// Transaction Result may be Success or Revert.
///
///  # Arguments
///
/// * `txs: Vec<TransactionRequest>` - Vector of transaction to be executed.
/// * `socket: &ExecutionEngineSocket` - Socket for submitting transaction.
///
/// # Returns
///
/// * `BlockExecutionResponse`
macro_rules! run_transactions {
    ($txs:expr,$socket:expr) => {{
        let result = run_transaction($txs, $socket).await;
        assert!(result.is_ok());
        result.unwrap()
    }};
}

/// Helper macro executing a single Update within a single Block.
/// Asserts that submission occurred.
/// Asserts that the Update was successful - `TransactionResponse::Success`.
///
///  # Arguments
///
/// * `update: UpdateRequest` - Vector of update requests to be executed.
/// * `socket: &ExecutionEngineSocket` - Socket for submitting transaction.
/// * `response: ExecutionData` - Expected execution data, optional param
///
/// # Returns
///
/// * `BlockExecutionResponse`
macro_rules! expect_tx_success {
    ($update:expr,$socket:expr) => {{
        expect_tx_success!($update, $socket, ExecutionData::None);
    }};
    ($update:expr,$socket:expr,$response:expr) => {{
        let result = run_update!($update, $socket);
        assert_eq!(
            result.txn_receipts[0].response,
            TransactionResponse::Success($response)
        );
        result
    }};
}

/// Helper macro executing a single Update within a single Block.
/// Asserts that submission occurred.
/// Asserts that the Update was reverted - `TransactionResponse::Revert`.
///
///  # Arguments
///
/// * `update: UpdateRequest` - Vector of update requests to be executed.
/// * `socket: &ExecutionEngineSocket` - Socket for submitting transaction.
/// * `revert: ExecutionError` - Expected execution error
macro_rules! expect_tx_revert {
    ($update:expr,$socket:expr,$revert:expr) => {{
        let result = run_update!($update, $socket);
        assert_eq!(
            result.txn_receipts[0].response,
            TransactionResponse::Revert($revert)
        );
    }};
}

/// Helper macro executing `ChangeEpoch` Update within a single Block.
/// Asserts that submission occurred.
///
///  # Arguments
///
/// * `socket: &ExecutionEngineSocket` - Socket for submitting transaction.
/// * `secret_key: &NodeSecretKey` - Node's secret key for signing transaction.
/// * `nonce: u64` - Nonce for Node's account.
/// * `epoch: u64` - Epoch to be changed.
///
/// # Returns
///
/// * `BlockExecutionResponse`
macro_rules! change_epoch {
    ($socket:expr,$secret_key:expr,$nonce:expr,$epoch:expr) => {{
        let req = prepare_update_request_node(
            UpdateMethod::ChangeEpoch { epoch: $epoch },
            $secret_key,
            $nonce,
        );
        run_update!(req, $socket)
    }};
}

/// Helper macro that performs an epoch change.
/// Asserts that submission occurred.
///
///  # Arguments
///
/// * `socket: &ExecutionEngineSocket` - Socket for submitting transaction.
/// * `committee_keystore: &Vec<GenesisCommitteeKeystore> ` - Keystore with committee's private
///   keys.
/// * `query_runner: &QueryRunner` - Query Runner.
/// * `epoch: u64` - Epoch to be changed.
macro_rules! simple_epoch_change {
    ($socket:expr,$committee_keystore:expr,$query_runner:expr,$epoch:expr) => {{
        let required_signals = calculate_required_signals($committee_keystore.len());
        // make call epoch change for 2/3rd committee members
        for (index, node) in $committee_keystore
            .iter()
            .enumerate()
            .take(required_signals)
        {
            let nonce = $query_runner
                .get_node_info(&node.node_secret_key.to_pk())
                .unwrap()
                .nonce
                + 1;
            let req = prepare_change_epoch_request($epoch, &node.node_secret_key, nonce);

            let res = run_update!(req, $socket);
            // check epoch change
            if index == required_signals - 1 {
                assert!(res.change_epoch);
            }
        }
    }};
}

/// Helper macro executing `SubmitReputationMeasurements` Update within a single Block.
/// Asserts that submission occurred.
/// Asserts that the Update was successful - `TransactionResponse::Success`.
///
///  # Arguments
///
/// * `socket: &ExecutionEngineSocket` - Socket for submitting transaction.
/// * `secret_key: &NodeSecretKey` - Node's secret key for signing transaction.
/// * `nonce: u64` - Nonce for Node's account.
/// * `measurements: BTreeMap<u32, ReputationMeasurements>` - Reputation measurements to be
///   submitted.
macro_rules! submit_reputation_measurements {
    ($socket:expr,$secret_key:expr,$nonce:expr,$measurements:expr) => {{
        let req = prepare_update_request_node(
            UpdateMethod::SubmitReputationMeasurements {
                measurements: $measurements,
            },
            $secret_key,
            $nonce,
        );
        expect_tx_success!(req, $socket)
    }};
}

/// Helper macro executing `SubmitReputationMeasurements` Update within a single Block.
/// Asserts that submission occurred.
/// Asserts that the Update was successful - `TransactionResponse::Success`.
///
///  # Arguments
///
/// * `socket: &ExecutionEngineSocket` - Socket for submitting transaction.
/// * `secret_key: &AccountOwnerSecretKey` - Account's secret key for signing transaction.
/// * `nonce: u64` - Nonce for the account.
/// * `amount: &HpUfixed<18>` - Amount to be deposited.
macro_rules! deposit {
    ($socket:expr,$secret_key:expr,$nonce:expr,$amount:expr) => {{
        let req = prepare_deposit_update($amount, $secret_key, $nonce);
        expect_tx_success!(req, $socket)
    }};
}

/// Helper macro executing `Stake` Update within a single Block.
/// Asserts that submission occurred.
/// Asserts that the Update was successful - `TransactionResponse::Success`.
///
///  # Arguments
///
/// * `socket: &ExecutionEngineSocket` - Socket for submitting transaction.
/// * `secret_key: &AccountOwnerSecretKey` - Account's secret key for signing transaction.
/// * `nonce: u64` - Nonce for the account.
/// * `amount: &HpUfixed<18>` - Amount to be staked.
/// * `node_pk: &NodePublicKey` - Public key of a Node to be staked on.
/// * `consensus_key: ConsensusPublicKey` - Consensus public key.
macro_rules! stake {
    ($socket:expr,$secret_key:expr,$nonce:expr,$amount:expr,$node_pk:expr,$consensus_key:expr) => {{
        let req = prepare_initial_stake_update(
            $amount,
            $node_pk,
            $consensus_key,
            "127.0.0.1".parse().unwrap(),
            [0; 32].into(),
            "127.0.0.1".parse().unwrap(),
            NodePorts::default(),
            $secret_key,
            $nonce,
        );

        expect_tx_success!(req, $socket)
    }};
}

/// Helper macro executing `Deposit` and `Stake` Updates within a single Block.
/// Asserts that submission occurred.
/// Asserts that Updates were successful - `TransactionResponse::Success`.
///
///  # Arguments
///
/// * `socket: &ExecutionEngineSocket` - Socket for submitting transaction.
/// * `secret_key: &AccountOwnerSecretKey` - Account's secret key for signing transaction.
/// * `nonce: u64` - Nonce for the account.
/// * `amount: &HpUfixed<18>` - Amount to be deposited and staked.
/// * `node_pk: &NodePublicKey` - Public key of a Node to be staked on.
/// * `consensus_key: ConsensusPublicKey` - Consensus public key.
macro_rules! deposit_and_stake {
    ($socket:expr,$secret_key:expr,$nonce:expr,$amount:expr,$node_pk:expr,$consensus_key:expr) => {{
        deposit!($socket, $secret_key, $nonce, $amount);
        stake!(
            $socket,
            $secret_key,
            $nonce + 1,
            $amount,
            $node_pk,
            $consensus_key
        );
    }};
}

/// Helper macro executing `StakeLock` Update within a single Block.
/// Asserts that submission occurred.
/// Asserts that the Updates was successful - `TransactionResponse::Success`.
///
///  # Arguments
///
/// * `socket: &ExecutionEngineSocket` - Socket for submitting transaction.
/// * `secret_key: &AccountOwnerSecretKey` - Account's secret key for signing transaction.
/// * `nonce: u64` - Nonce for the account.
/// * `node_pk: &NodePublicKey` - Public key of a Node.
/// * `locked_for: u64` - Lock time.
macro_rules! stake_lock {
    ($socket:expr,$secret_key:expr,$nonce:expr,$node_pk:expr,$locked_for:expr) => {{
        let req = prepare_stake_lock_request($locked_for, $node_pk, $secret_key, $nonce);
        expect_tx_success!(req, $socket)
    }};
}

/// Assert that Reputation Measurements are submitted (updated).
///
///  # Arguments
///
/// * `query_runner: &QueryRunner` - Query Runner.
/// * `update: (u32, ReputationMeasurements)` - Tuple containing node index and reputation
///   measurements.
/// * `reporting_node_index: u64` - Reporting Node index.
macro_rules! assert_rep_measurements_update {
    ($query_runner:expr,$update:expr,$reporting_node_index:expr) => {{
        let rep_measurements = $query_runner.get_rep_measurements(&$update.0);
        assert_eq!(rep_measurements.len(), 1);
        assert_eq!(rep_measurements[0].reporting_node, $reporting_node_index);
        assert_eq!(rep_measurements[0].measurements, $update.1);
    }};
}

/// Assert that a Node is valid.
///
///  # Arguments
///
/// * `valid_nodes: &Vec<NodeInfo>` - List of valid nodes.
/// * `query_runner: &QueryRunner` - Query Runner.
/// * `node_pk: &NodePublicKey` - Node's public key
macro_rules! assert_valid_node {
    ($valid_nodes:expr,$query_runner:expr,$node_pk:expr) => {{
        let node_info = $query_runner.get_node_info($node_pk).unwrap();
        // Node registry contains the first valid node
        assert!($valid_nodes.contains(&node_info));
    }};
}

/// Assert that a Node is NOT valid.
///
///  # Arguments
///
/// * `valid_nodes: &Vec<NodeInfo>` - List of valid nodes.
/// * `query_runner: &QueryRunner` - Query Runner.
/// * `node_pk: &NodePublicKey` - Node's public key
macro_rules! assert_not_valid_node {
    ($valid_nodes:expr,$query_runner:expr,$node_pk:expr) => {{
        let node_info = $query_runner.get_node_info($node_pk).unwrap();
        // Node registry contains the first valid node
        assert!(!$valid_nodes.contains(&node_info));
    }};
}

/// Assert that paging works properly with `get_node_registry`.
///
///  # Arguments
///
/// * `query_runner: &QueryRunner` - Query Runner.
/// * `paging_params: PagingParams` - Paging params.
/// * `expected_len: usize` - Expected length of the query result.
macro_rules! assert_paging_node_registry {
    ($query_runner:expr,$paging_params:expr, $expected_len:expr) => {{
        let valid_nodes = $query_runner.get_node_registry(Some($paging_params));
        assert_eq!(valid_nodes.len(), $expected_len);
    }};
}

/// Load Tests Genesis configuration
fn test_genesis() -> Genesis {
    Genesis::load().expect("Failed to load genesis from file.")
}

/// Initialize application state with provided or default configuration.
fn init_app(config: Option<Config>) -> (ExecutionEngineSocket, QueryRunner) {
    let config = config.or(Some(Config {
        genesis: None,
        mode: Mode::Dev,
        testnet: false,
        storage: StorageConfig::InMemory,
        db_path: None,
        db_options: None,
    }));
    do_init_app(config.unwrap())
}

/// Initialize application with provided configuration.
fn do_init_app(config: Config) -> (ExecutionEngineSocket, QueryRunner) {
    let app = Application::<TestBinding>::init(config, Default::default()).unwrap();

    (app.transaction_executor(), app.sync_query())
}

/// Initialize application with provided committee.
fn test_init_app(committee: Vec<GenesisNode>) -> (ExecutionEngineSocket, QueryRunner) {
    let mut genesis = test_genesis();
    genesis.node_info = committee;
    init_app(Some(test_config(genesis)))
}

/// Initialize application with provided genesis.
fn init_app_with_genesis(genesis: &Genesis) -> (ExecutionEngineSocket, QueryRunner) {
    init_app(Some(test_config(genesis.clone())))
}

/// Initialize application with provided parameters.
fn init_app_with_params(
    params: Params,
    committee: Option<Vec<GenesisNode>>,
) -> (ExecutionEngineSocket, QueryRunner) {
    let mut genesis = test_genesis();

    if let Some(committee) = committee {
        genesis.node_info = committee;
    }

    genesis.epoch_start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    if let Some(epoch_time) = params.epoch_time {
        genesis.epoch_time = epoch_time;
    }

    if let Some(max_inflation) = params.max_inflation {
        genesis.max_inflation = max_inflation;
    }

    if let Some(protocol_share) = params.protocol_share {
        genesis.protocol_share = protocol_share;
    }

    if let Some(node_share) = params.node_share {
        genesis.node_share = node_share;
    }

    if let Some(service_builder_share) = params.service_builder_share {
        genesis.service_builder_share = service_builder_share;
    }

    if let Some(max_boost) = params.max_boost {
        genesis.max_boost = max_boost;
    }

    if let Some(supply_at_genesis) = params.supply_at_genesis {
        genesis.supply_at_genesis = supply_at_genesis;
    }
    let config = Config {
        genesis: Some(genesis),
        mode: Mode::Test,
        testnet: false,
        storage: StorageConfig::InMemory,
        db_path: None,
        db_options: None,
    };

    init_app(Some(config))
}

/// Prepare test Config based on provided genesis.
fn test_config(genesis: Genesis) -> Config {
    Config {
        genesis: Some(genesis),
        mode: Mode::Test,
        testnet: false,
        storage: StorageConfig::InMemory,
        db_path: None,
        db_options: None,
    }
}

/// Prepare test Reputation Measurements based on provided `uptime`.
fn test_reputation_measurements(uptime: u8) -> ReputationMeasurements {
    ReputationMeasurements {
        latency: None,
        interactions: None,
        inbound_bandwidth: None,
        outbound_bandwidth: None,
        bytes_received: None,
        bytes_sent: None,
        uptime: Some(HpFixed::from(uptime as i32)),
        hops: None,
    }
}

/// Calculate requited signals for epoch change
fn calculate_required_signals(committee_size: usize) -> usize {
    2 * committee_size / 3 + 1
}

/// Create a test genesis committee.
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
            consensus_secret_key,
        });
    });
    (committee, keystore)
}

/// Create a new member for test committee.
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

/// Prepare an `UpdateRequest` from an `UpdateMethod` signed with `NodeSecretKey`.
/// Passing the private key around like this should only be done for testing.
fn prepare_update_request_node(
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

/// Prepare an `UpdateRequest` from an `UpdateMethod` signed with `ConsensusSecretKey`.
/// Passing the private key around like this should only be done for testing.
fn prepare_update_request_consensus(
    method: UpdateMethod,
    secret_key: &ConsensusSecretKey,
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

/// Prepare an `UpdateRequest` from an `UpdateMethod` signed with `AccountOwnerSecretKey`.
/// Passing the private key around like this should only be done for testing.
fn prepare_update_request_account(
    method: UpdateMethod,
    secret_key: &AccountOwnerSecretKey,
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

/// Prepare an `UpdateRequest` for `UpdateMethod::Deposit` signed with `AccountOwnerSecretKey`.
/// Passing the private key around like this should only be done for testing.
fn prepare_deposit_update(
    amount: &HpUfixed<18>,
    secret_key: &AccountOwnerSecretKey,
    nonce: u64,
) -> UpdateRequest {
    prepare_update_request_account(
        UpdateMethod::Deposit {
            proof: ProofOfConsensus {},
            token: Tokens::FLK,
            amount: amount.clone(),
        },
        secret_key,
        nonce,
    )
}

/// Prepare an `UpdateRequest` for `UpdateMethod::Stake` signed with `AccountOwnerSecretKey`.
/// For the first `Stake`, use `prepare_initial_stake_update`.
/// Passing the private key around like this should only be done for testing.
fn prepare_regular_stake_update(
    amount: &HpUfixed<18>,
    node_public_key: &NodePublicKey,
    secret_key: &AccountOwnerSecretKey,
    nonce: u64,
) -> UpdateRequest {
    prepare_update_request_account(
        UpdateMethod::Stake {
            amount: amount.clone(),
            node_public_key: *node_public_key,
            consensus_key: None,
            node_domain: None,
            worker_public_key: None,
            worker_domain: None,
            ports: None,
        },
        secret_key,
        nonce,
    )
}

/// Prepare an `UpdateRequest` for `UpdateMethod::Stake` signed with `AccountOwnerSecretKey`.
/// Passing the private key around like this should only be done for testing.
#[allow(clippy::too_many_arguments)]
fn prepare_initial_stake_update(
    amount: &HpUfixed<18>,
    node_public_key: &NodePublicKey,
    consensus_key: ConsensusPublicKey,
    node_domain: IpAddr,
    worker_pub_key: NodePublicKey,
    worker_domain: IpAddr,
    ports: NodePorts,
    secret_key: &AccountOwnerSecretKey,
    nonce: u64,
) -> UpdateRequest {
    prepare_update_request_account(
        UpdateMethod::Stake {
            amount: amount.clone(),
            node_public_key: *node_public_key,
            consensus_key: Some(consensus_key),
            node_domain: Some(node_domain),
            worker_public_key: Some(worker_pub_key),
            worker_domain: Some(worker_domain),
            ports: Some(ports),
        },
        secret_key,
        nonce,
    )
}

/// Prepare an `UpdateRequest` for `UpdateMethod::Unstake` signed with `AccountOwnerSecretKey`.
/// Passing the private key around like this should only be done for testing.
fn prepare_unstake_update(
    amount: &HpUfixed<18>,
    node_public_key: &NodePublicKey,
    secret_key: &AccountOwnerSecretKey,
    nonce: u64,
) -> UpdateRequest {
    prepare_update_request_account(
        UpdateMethod::Unstake {
            amount: amount.clone(),
            node: *node_public_key,
        },
        secret_key,
        nonce,
    )
}

/// Prepare an `UpdateRequest` for `UpdateMethod::WithdrawUnstaked` signed with
/// `AccountOwnerSecretKey`. Passing the private key around like this should only be done for
/// testing.
fn prepare_withdraw_unstaked_update(
    node_public_key: &NodePublicKey,
    recipient: Option<EthAddress>,
    secret_key: &AccountOwnerSecretKey,
    nonce: u64,
) -> UpdateRequest {
    prepare_update_request_account(
        UpdateMethod::WithdrawUnstaked {
            node: *node_public_key,
            recipient,
        },
        secret_key,
        nonce,
    )
}

/// Prepare an `UpdateRequest` for `UpdateMethod::StakeLock` signed with `AccountOwnerSecretKey`.
/// Passing the private key around like this should only be done for testing.
fn prepare_stake_lock_update(
    node_public_key: &NodePublicKey,
    locked_for: u64,
    secret_key: &AccountOwnerSecretKey,
    nonce: u64,
) -> UpdateRequest {
    prepare_update_request_account(
        UpdateMethod::StakeLock {
            node: *node_public_key,
            locked_for,
        },
        secret_key,
        nonce,
    )
}

/// Prepare an `UpdateRequest` for `UpdateMethod::SubmitDeliveryAcknowledgmentAggregation` signed
/// with `NodeSecretKey`. Passing the private key around like this should only be done for testing.
fn prepare_pod_request(
    commodity: u128,
    service_id: u32,
    secret_key: &NodeSecretKey,
    nonce: u64,
) -> UpdateRequest {
    prepare_update_request_node(
        UpdateMethod::SubmitDeliveryAcknowledgmentAggregation {
            commodity,  // units of data served
            service_id, // service 0 serving bandwidth
            proofs: vec![DeliveryAcknowledgment],
            metadata: None,
        },
        secret_key,
        nonce,
    )
}

/// Prepare an `UpdateRequest` for `UpdateMethod::SubmitDeliveryAcknowledgmentAggregation` signed
/// with `AccountOwnerSecretKey`. Passing the private key around like this should only be done for
/// testing.
fn prepare_stake_lock_request(
    locked_for: u64,
    node: &NodePublicKey,
    secret_key: &AccountOwnerSecretKey,
    nonce: u64,
) -> UpdateRequest {
    // Deposit some FLK into account 1
    prepare_update_request_account(
        UpdateMethod::StakeLock {
            node: *node,
            locked_for,
        },
        secret_key,
        nonce,
    )
}

/// Prepare an `UpdateRequest` for `UpdateMethod::ChangeEpoch` signed with `NodeSecretKey`.
/// Passing the private key around like this should only be done for testing.
fn prepare_change_epoch_request(
    epoch: u64,
    secret_key: &NodeSecretKey,
    nonce: u64,
) -> UpdateRequest {
    prepare_update_request_node(UpdateMethod::ChangeEpoch { epoch }, secret_key, nonce)
}

/// Prepare an `UpdateRequest` for `UpdateMethod::Transfer` signed with `AccountOwnerSecretKey`.
/// Passing the private key around like this should only be done for testing.
fn prepare_transfer_request(
    amount: &HpUfixed<18>,
    to: &EthAddress,
    secret_key: &AccountOwnerSecretKey,
    nonce: u64,
) -> UpdateRequest {
    prepare_update_request_account(
        UpdateMethod::Transfer {
            amount: amount.clone(),
            token: Tokens::FLK,
            to: *to,
        },
        secret_key,
        nonce,
    )
}

/// Prepare an `UpdateRequest` for `UpdateMethod::ChangeProtocolParam` signed with
/// `AccountOwnerSecretKey`. Passing the private key around like this should only be done for
/// testing.
fn prepare_change_protocol_param_request(
    param: &ProtocolParams,
    value: &u128,
    secret_key: &AccountOwnerSecretKey,
    nonce: u64,
) -> UpdateRequest {
    prepare_update_request_account(
        UpdateMethod::ChangeProtocolParam {
            param: param.clone(),
            value: *value,
        },
        secret_key,
        nonce,
    )
}

/// Helper (async) function that submit a transaction to the application via `UpdateSocket`.
/// Returns `Result<BlockExecutionResponse>`.
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

/// Helper function that update `BTreeMap<u32, ReputationMeasurements>` with new
/// `ReputationMeasurements` for given `NodePublicKey` Returns tuple `(peer_index, measurements)`.
fn update_reputation_measurements(
    query_runner: &QueryRunner,
    map: &mut BTreeMap<u32, ReputationMeasurements>,
    peer: &NodePublicKey,
    measurements: ReputationMeasurements,
) -> (u32, ReputationMeasurements) {
    let peer_index = query_runner.pubkey_to_index(*peer).unwrap();
    map.insert(peer_index, measurements.clone());
    (peer_index, measurements)
}

/// Helper function that prepare `PagingParams`
fn paging_params(ignore_stake: bool, start: u32, limit: usize) -> PagingParams {
    PagingParams {
        ignore_stake,
        start,
        limit,
    }
}

/// Helper function that add a node to the `committee`.
fn add_to_committee(
    committee: &mut Vec<GenesisNode>,
    keystore: &mut Vec<GenesisCommitteeKeystore>,
    node_secret_key: NodeSecretKey,
    consensus_secret_key: ConsensusSecretKey,
    owner_secret_key: AccountOwnerSecretKey,
    index: u16,
) {
    let node_public_key = node_secret_key.to_pk();
    let consensus_public_key = consensus_secret_key.to_pk();
    let owner_public_key = owner_secret_key.to_pk();
    committee.push(GenesisNode::new(
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
            pinger: 2000 + index,
            handshake: HandshakePorts {
                http: 5000 + index,
                webrtc: 6000 + index,
                webtransport: 7000 + index,
            },
        },
        None,
        true,
    ));
    keystore.push(GenesisCommitteeKeystore {
        _owner_secret_key: owner_secret_key,
        _worker_secret_key: node_secret_key.clone(),
        node_secret_key,
        consensus_secret_key,
    });
}

/// Helper function that prepare new `committee`.
fn prepare_new_committee(
    query_runner: &QueryRunner,
    committee: &[GenesisNode],
    keystore: &[GenesisCommitteeKeystore],
) -> (Vec<GenesisNode>, Vec<GenesisCommitteeKeystore>) {
    let mut new_committee = Vec::new();
    let mut new_keystore = Vec::new();
    let committee_members = query_runner.get_committee_members();
    for node in committee_members {
        let index = committee
            .iter()
            .enumerate()
            .find_map(|(index, c)| {
                if c.primary_public_key == node {
                    Some(index)
                } else {
                    None
                }
            })
            .expect("Committee member was not found in genesis Committee");
        new_committee.push(committee[index].clone());
        new_keystore.push(keystore[index].clone());
    }
    (new_committee, new_keystore)
}

//////////////////////////////////////////////////////////////////////////////////
////////////////// This is where the actual tests are defined ////////////////////
//////////////////////////////////////////////////////////////////////////////////

#[tokio::test]
async fn test_genesis_configuration() {
    // Init application + get the query and update socket
    let (_, query_runner) = init_app(None);
    // Get the genesis parameters plus the initial committee
    let genesis = test_genesis();
    let genesis_committee = genesis.node_info;
    // For every member of the genesis committee they should have an initial stake of the min stake
    // Query to make sure that holds true
    for node in genesis_committee {
        let balance = query_runner.get_staked(&node.primary_public_key);
        assert_eq!(HpUfixed::<18>::from(genesis.min_stake), balance);
    }
}

#[tokio::test]
async fn test_epoch_change() {
    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(committee);
    let required_signals = calculate_required_signals(committee_size);

    let epoch = 0;
    let nonce = 1;

    // Have (required_signals - 1) say they are ready to change epoch
    // make sure the epoch doesn't change each time someone signals
    for node in keystore.iter().take(required_signals - 1) {
        // Make sure epoch didn't change
        let res = change_epoch!(&update_socket, &node.node_secret_key, nonce, epoch);
        assert!(!res.change_epoch);
    }
    // check that the current epoch is still 0
    assert_eq!(query_runner.get_epoch_info().epoch, 0);

    // Have the last needed committee member signal the epoch change and make sure it changes
    let res = change_epoch!(
        &update_socket,
        &keystore[required_signals].node_secret_key,
        nonce,
        epoch
    );
    assert!(res.change_epoch);

    // Query epoch info and make sure it incremented to new epoch
    assert_eq!(query_runner.get_epoch_info().epoch, 1);
}

#[tokio::test]
async fn test_submit_rep_measurements() {
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(committee);
    let mut rng = random::get_seedable_rng();

    let mut map = BTreeMap::new();
    let update1 = update_reputation_measurements(
        &query_runner,
        &mut map,
        &keystore[1].node_secret_key.to_pk(),
        reputation::generate_reputation_measurements(&mut rng, 0.1),
    );
    let update2 = update_reputation_measurements(
        &query_runner,
        &mut map,
        &keystore[2].node_secret_key.to_pk(),
        reputation::generate_reputation_measurements(&mut rng, 0.1),
    );

    let reporting_node_key = keystore[0].node_secret_key.to_pk();
    let reporting_node_index = query_runner.pubkey_to_index(reporting_node_key).unwrap();

    submit_reputation_measurements!(&update_socket, &keystore[0].node_secret_key, 1, map);

    assert_rep_measurements_update!(&query_runner, update1, reporting_node_index);
    assert_rep_measurements_update!(&query_runner, update2, reporting_node_index);
}

#[tokio::test]
async fn test_submit_rep_measurements_too_many_times() {
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(committee);

    let mut rng = random::get_seedable_rng();

    let mut map = BTreeMap::new();
    let _ = update_reputation_measurements(
        &query_runner,
        &mut map,
        &keystore[1].node_secret_key.to_pk(),
        reputation::generate_reputation_measurements(&mut rng, 0.1),
    );

    // Attempt to submit reputation measurements 1 more time than allowed per epoch.
    // This transaction should revert because each node only can submit its reputation measurements
    // `MAX_MEASUREMENTS_SUBMIT` times.
    for i in 0..MAX_MEASUREMENTS_SUBMIT {
        let req = prepare_update_request_node(
            UpdateMethod::SubmitReputationMeasurements {
                measurements: map.clone(),
            },
            &keystore[0].node_secret_key,
            1 + i as u64,
        );
        expect_tx_success!(req, &update_socket);
    }
    let req = prepare_update_request_node(
        UpdateMethod::SubmitReputationMeasurements { measurements: map },
        &keystore[0].node_secret_key,
        1 + MAX_MEASUREMENTS_SUBMIT as u64,
    );
    expect_tx_revert!(
        req,
        &update_socket,
        ExecutionError::SubmittedTooManyTransactions
    );
}

#[tokio::test]
async fn test_rep_scores() {
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(committee);
    let required_signals = calculate_required_signals(committee_size);

    let mut rng = random::get_seedable_rng();

    let peer1 = keystore[2].node_secret_key.to_pk();
    let peer2 = keystore[3].node_secret_key.to_pk();
    let nonce = 1;

    let mut map = BTreeMap::new();
    let _ = update_reputation_measurements(
        &query_runner,
        &mut map,
        &peer1,
        reputation::generate_reputation_measurements(&mut rng, 0.1),
    );
    let _ = update_reputation_measurements(
        &query_runner,
        &mut map,
        &peer2,
        reputation::generate_reputation_measurements(&mut rng, 0.1),
    );
    submit_reputation_measurements!(&update_socket, &keystore[0].node_secret_key, nonce, map);

    let mut map = BTreeMap::new();
    let (peer_idx_1, _) = update_reputation_measurements(
        &query_runner,
        &mut map,
        &peer1,
        reputation::generate_reputation_measurements(&mut rng, 0.1),
    );
    let (peer_idx_2, _) = update_reputation_measurements(
        &query_runner,
        &mut map,
        &peer2,
        reputation::generate_reputation_measurements(&mut rng, 0.1),
    );
    submit_reputation_measurements!(&update_socket, &keystore[1].node_secret_key, nonce, map);

    let epoch = 0;
    // Change epoch so that rep scores will be calculated from the measurements.
    for (i, node) in keystore.iter().enumerate().take(required_signals) {
        // Not the prettiest solution but we have to keep track of the nonces somehow.
        let nonce = if i < 2 { 2 } else { 1 };
        change_epoch!(&update_socket, &node.node_secret_key, nonce, epoch);
    }

    assert!(query_runner.get_reputation(&peer_idx_1).is_some());
    assert!(query_runner.get_reputation(&peer_idx_2).is_some());
}

#[tokio::test]
async fn test_uptime_participation() {
    let committee_size = 4;
    let (mut committee, keystore) = create_genesis_committee(committee_size);
    committee[0].reputation = Some(40);
    committee[1].reputation = Some(80);
    let (update_socket, query_runner) = test_init_app(committee);

    let required_signals = calculate_required_signals(committee_size);

    let peer_1 = keystore[2].node_secret_key.to_pk();
    let peer_2 = keystore[3].node_secret_key.to_pk();
    let nonce = 1;

    let mut map = BTreeMap::new();
    let _ = update_reputation_measurements(
        &query_runner,
        &mut map,
        &peer_1,
        test_reputation_measurements(20),
    );
    let _ = update_reputation_measurements(
        &query_runner,
        &mut map,
        &peer_2,
        test_reputation_measurements(40),
    );

    submit_reputation_measurements!(&update_socket, &keystore[0].node_secret_key, nonce, map);

    let mut map = BTreeMap::new();
    let _ = update_reputation_measurements(
        &query_runner,
        &mut map,
        &peer_1,
        test_reputation_measurements(30),
    );

    let _ = update_reputation_measurements(
        &query_runner,
        &mut map,
        &peer_2,
        test_reputation_measurements(45),
    );
    submit_reputation_measurements!(&update_socket, &keystore[1].node_secret_key, nonce, map);

    let epoch = 0;
    // Change epoch so that rep scores will be calculated from the measurements.
    for (i, node) in keystore.iter().enumerate().take(required_signals) {
        // Not the prettiest solution but we have to keep track of the nonces somehow.
        let nonce = if i < 2 { 2 } else { 1 };
        change_epoch!(&update_socket, &node.node_secret_key, nonce, epoch);
    }

    let node_info1 = query_runner.get_node_info(&peer_1).unwrap();
    let node_info2 = query_runner.get_node_info(&peer_2).unwrap();

    assert_eq!(node_info1.participation, Participation::False);
    assert_eq!(node_info2.participation, Participation::True);
}

#[tokio::test]
async fn test_stake() {
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let peer_pub_key = NodeSecretKey::generate().to_pk();

    // Deposit some FLK into account 1
    let deposit = 1000_u64.into();
    let update1 = prepare_deposit_update(&deposit, &owner_secret_key, 1);
    let update2 = prepare_deposit_update(&deposit, &owner_secret_key, 2);

    // Put 2 of the transaction in the block just to also test block exucution a bit
    let _ = run_updates!(vec![update1, update2], &update_socket);

    // check that he has 2_000 flk balance
    assert_eq!(
        query_runner.get_flk_balance(&owner_secret_key.to_pk().into()),
        (HpUfixed::<18>::from(2u16) * deposit)
    );

    // Test staking on a new node
    let stake_amount = 1000u64.into();
    // First check that trying to stake without providing all the node info reverts
    let update = prepare_regular_stake_update(&stake_amount, &peer_pub_key, &owner_secret_key, 3);
    expect_tx_revert!(
        update,
        &update_socket,
        ExecutionError::InsufficientNodeDetails
    );

    // Now try with the correct details for a new node
    let update = prepare_initial_stake_update(
        &stake_amount,
        &peer_pub_key,
        [0; 96].into(),
        "127.0.0.1".parse().unwrap(),
        [0; 32].into(),
        "127.0.0.1".parse().unwrap(),
        NodePorts::default(),
        &owner_secret_key,
        4,
    );

    expect_tx_success!(update, &update_socket);

    // Query the new node and make sure he has the proper stake
    assert_eq!(query_runner.get_staked(&peer_pub_key), stake_amount);

    // Stake 1000 more but since it is not a new node we should be able to leave the optional
    // parameters out without a revert
    let update = prepare_regular_stake_update(&stake_amount, &peer_pub_key, &owner_secret_key, 5);

    expect_tx_success!(update, &update_socket);

    // Node should now have 2_000 stake
    assert_eq!(
        query_runner.get_staked(&peer_pub_key),
        (HpUfixed::<18>::from(2u16) * stake_amount.clone())
    );

    // Now test unstake and make sure it moves the tokens to locked status
    let update = prepare_unstake_update(&stake_amount, &peer_pub_key, &owner_secret_key, 6);
    run_update!(update, &update_socket);

    // Check that his locked is 1000 and his remaining stake is 1000
    assert_eq!(query_runner.get_staked(&peer_pub_key), stake_amount);
    assert_eq!(query_runner.get_locked(&peer_pub_key), stake_amount);

    // Since this test starts at epoch 0 locked_until will be == lock_time
    assert_eq!(
        query_runner.get_locked_time(&peer_pub_key),
        test_genesis().lock_time
    );

    // Try to withdraw the locked tokens and it should revert
    let update = prepare_withdraw_unstaked_update(&peer_pub_key, None, &owner_secret_key, 7);

    expect_tx_revert!(update, &update_socket, ExecutionError::TokensLocked);
}

#[tokio::test]
async fn test_stake_lock() {
    let (update_socket, query_runner) = init_app(None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let amount: HpUfixed<18> = 1_000u64.into();

    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &amount,
        &node_pub_key,
        [0; 96].into()
    );

    assert_eq!(query_runner.get_staked(&node_pub_key), amount);

    let locked_for = 365;
    let stake_lock_req = prepare_stake_lock_update(&node_pub_key, locked_for, &owner_secret_key, 3);

    expect_tx_success!(stake_lock_req, &update_socket);

    assert_eq!(
        query_runner.get_stake_locked_until(&node_pub_key),
        locked_for
    );

    let unstake_req = prepare_unstake_update(&amount, &node_pub_key, &owner_secret_key, 4);
    expect_tx_revert!(
        unstake_req,
        &update_socket,
        ExecutionError::LockedTokensUnstakeForbidden
    );
}

#[tokio::test]
async fn test_pod_without_proof() {
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(committee);

    let bandwidth_commodity = 1000;
    let compute_commodity = 2000;
    let bandwidth_pod =
        prepare_pod_request(bandwidth_commodity, 0, &keystore[0].node_secret_key, 1);
    let compute_pod = prepare_pod_request(compute_commodity, 1, &keystore[0].node_secret_key, 2);

    // run the delivery ack transaction
    run_updates!(vec![bandwidth_pod, compute_pod], &update_socket);

    assert_eq!(
        query_runner
            .get_node_served(&keystore[0].node_secret_key.to_pk())
            .served,
        vec![bandwidth_commodity, compute_commodity]
    );

    assert_eq!(
        query_runner.get_total_served(0),
        TotalServed {
            served: vec![bandwidth_commodity, compute_commodity],
            reward_pool: (0.1 * bandwidth_commodity as f64 + 0.2 * compute_commodity as f64).into()
        }
    );
}

#[tokio::test]
async fn test_is_valid_node() {
    let (update_socket, query_runner) = init_app(None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();

    // Stake minimum required amount.
    let minimum_stake_amount = query_runner.get_staking_amount().into();
    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &minimum_stake_amount,
        &node_pub_key,
        [0; 96].into()
    );

    // Make sure that this node is a valid node.
    assert!(query_runner.is_valid_node(&node_pub_key));

    // Generate new keys for a different node.
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();

    // Stake less than the minimum required amount.
    let less_than_minimum_skate_amount = minimum_stake_amount / HpUfixed::<18>::from(2u16);
    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &less_than_minimum_skate_amount,
        &node_pub_key,
        [1; 96].into()
    );
    // Make sure that this node is not a valid node.
    assert!(!query_runner.is_valid_node(&node_pub_key));
}

#[tokio::test]
async fn test_change_protocol_params() {
    let governance_secret_key = AccountOwnerSecretKey::generate();
    let governance_public_key = governance_secret_key.to_pk();

    let mut genesis = test_genesis();
    genesis.governance_address = governance_public_key.into();

    let (update_socket, query_runner) = init_app_with_genesis(&genesis);

    let param = ProtocolParams::LockTime;
    let new_value = 5;
    let update =
        prepare_change_protocol_param_request(&param, &new_value, &governance_secret_key, 1);
    run_update!(update, &update_socket);
    assert_eq!(query_runner.get_protocol_params(param.clone()), new_value);

    let new_value = 8;
    let update =
        prepare_change_protocol_param_request(&param, &new_value, &governance_secret_key, 2);
    run_update!(update, &update_socket);
    assert_eq!(query_runner.get_protocol_params(param.clone()), new_value);

    // Make sure that another private key cannot change protocol parameters.
    let some_secret_key = AccountOwnerSecretKey::generate();
    let minimum_stake_amount = query_runner.get_staking_amount().into();
    deposit!(&update_socket, &some_secret_key, 1, &minimum_stake_amount);

    let malicious_value = 1;
    let update =
        prepare_change_protocol_param_request(&param, &malicious_value, &some_secret_key, 2);
    expect_tx_revert!(update, &update_socket, ExecutionError::OnlyGovernance);
    // Lock time should still be 8.
    assert_eq!(query_runner.get_protocol_params(param), new_value)
}

#[tokio::test]
async fn test_change_protocol_params_reverts_not_account_key() {
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(committee);

    let param = ProtocolParams::LockTime;
    let initial_value = query_runner.get_protocol_params(param.clone());
    let new_value = initial_value + 1;

    let change_method = UpdateMethod::ChangeProtocolParam {
        param: param.clone(),
        value: new_value,
    };

    // Assert that reverts for Node Key
    let update =
        prepare_update_request_node(change_method.clone(), &keystore[0].node_secret_key, 1);
    expect_tx_revert!(update, &update_socket, ExecutionError::OnlyAccountOwner);
    assert_eq!(
        query_runner.get_protocol_params(param.clone()),
        initial_value
    );

    // Assert that reverts for Consensus Key
    let update = prepare_update_request_consensus(
        change_method.clone(),
        &keystore[0].consensus_secret_key,
        2,
    );
    expect_tx_revert!(update, &update_socket, ExecutionError::OnlyAccountOwner);
    assert_eq!(
        query_runner.get_protocol_params(param.clone()),
        initial_value
    );
}

#[tokio::test]
async fn test_validate_txn() {
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(committee);

    // Submit a ChangeEpoch transaction that will revert (EpochHasNotStarted) and ensure that the
    // `validate_txn` method of the query runner returns the same response as the update runner.
    let invalid_epoch = 1;
    let req = prepare_change_epoch_request(invalid_epoch, &keystore[0].node_secret_key, 1);
    let res = run_update!(req, &update_socket);

    let req = prepare_change_epoch_request(invalid_epoch, &keystore[0].node_secret_key, 2);
    assert_eq!(
        res.txn_receipts[0].response,
        query_runner.validate_txn(req.into())
    );

    // Submit a ChangeEpoch transaction that will succeed and ensure that the
    // `validate_txn` method of the query runner returns the same response as the update runner.
    let epoch = 0;
    let req = prepare_change_epoch_request(epoch, &keystore[0].node_secret_key, 2);

    let res = run_update!(req, &update_socket);
    let req = prepare_change_epoch_request(epoch, &keystore[1].node_secret_key, 1);

    assert_eq!(
        res.txn_receipts[0].response,
        query_runner.validate_txn(req.into())
    );
}

#[tokio::test]
async fn test_distribute_rewards() {
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);

    let max_inflation = 10;
    let protocol_part = 10;
    let node_part = 80;
    let service_part = 10;
    let boost = 4;
    let supply_at_genesis = 1_000_000;
    let (update_socket, query_runner) = init_app_with_params(
        Params {
            epoch_time: None,
            max_inflation: Some(max_inflation),
            protocol_share: Some(protocol_part),
            node_share: Some(node_part),
            service_builder_share: Some(service_part),
            max_boost: Some(boost),
            supply_at_genesis: Some(supply_at_genesis),
        },
        Some(committee),
    );

    // get params for emission calculations
    let percentage_divisor: HpUfixed<18> = 100_u16.into();
    let supply_at_year_start: HpUfixed<18> = supply_at_genesis.into();
    let inflation: HpUfixed<18> = HpUfixed::from(max_inflation) / &percentage_divisor;
    let node_share = HpUfixed::from(node_part) / &percentage_divisor;
    let protocol_share = HpUfixed::from(protocol_part) / &percentage_divisor;
    let service_share = HpUfixed::from(service_part) / &percentage_divisor;

    let owner_secret_key1 = AccountOwnerSecretKey::generate();
    let node_secret_key1 = NodeSecretKey::generate();
    let owner_secret_key2 = AccountOwnerSecretKey::generate();
    let node_secret_key2 = NodeSecretKey::generate();

    let deposit_amount = 10_000_u64.into();
    let locked_for = 1460;
    // deposit FLK tokens and stake it
    deposit_and_stake!(
        &update_socket,
        &owner_secret_key1,
        1,
        &deposit_amount,
        &node_secret_key1.to_pk(),
        [0; 96].into()
    );
    deposit_and_stake!(
        &update_socket,
        &owner_secret_key2,
        1,
        &deposit_amount,
        &node_secret_key2.to_pk(),
        [1; 96].into()
    );
    stake_lock!(
        &update_socket,
        &owner_secret_key2,
        3,
        &node_secret_key2.to_pk(),
        locked_for
    );

    // submit pods for usage
    let commodity_10 = 12_800;
    let commodity_11 = 3_600;
    let commodity_21 = 5000;
    let pod_10 = prepare_pod_request(commodity_10, 0, &node_secret_key1, 1);
    let pod_11 = prepare_pod_request(commodity_11, 1, &node_secret_key1, 2);
    let pod_21 = prepare_pod_request(commodity_21, 1, &node_secret_key2, 1);

    let node_1_usd = 0.1 * (commodity_10 as f64) + 0.2 * (commodity_11 as f64); // 2_000 in revenue
    let node_2_usd = 0.2 * (commodity_21 as f64); // 1_000 in revenue
    let reward_pool: HpUfixed<6> = (node_1_usd + node_2_usd).into();

    let node_1_proportion: HpUfixed<18> = HpUfixed::from(2000_u64) / HpUfixed::from(3000_u64);
    let node_2_proportion: HpUfixed<18> = HpUfixed::from(1000_u64) / HpUfixed::from(3000_u64);

    let service_proportions: Vec<HpUfixed<18>> = vec![
        HpUfixed::from(1280_u64) / HpUfixed::from(3000_u64),
        HpUfixed::from(1720_u64) / HpUfixed::from(3000_u64),
    ];

    // run the delivery ack transaction
    run_updates!(vec![pod_10, pod_11, pod_21], &update_socket);

    // call epoch change that will trigger distribute rewards
    simple_epoch_change!(&update_socket, &keystore, &query_runner, 0);

    // assert stable balances
    assert_eq!(
        query_runner.get_stables_balance(&owner_secret_key1.to_pk().into()),
        HpUfixed::<6>::from(node_1_usd) * node_share.convert_precision()
    );
    assert_eq!(
        query_runner.get_stables_balance(&owner_secret_key2.to_pk().into()),
        HpUfixed::<6>::from(node_2_usd) * node_share.convert_precision()
    );

    let total_share =
        &node_1_proportion * HpUfixed::from(1_u64) + &node_2_proportion * HpUfixed::from(4_u64);

    // calculate emissions per unit
    let emissions: HpUfixed<18> = (inflation * supply_at_year_start) / &365.0.into();
    let emissions_for_node = &emissions * &node_share;

    // assert flk balances node 1
    assert_eq!(
        // node_flk_balance1
        query_runner.get_flk_balance(&owner_secret_key1.to_pk().into()),
        // node_flk_rewards1
        (&emissions_for_node * &node_1_proportion) / &total_share
    );

    // assert flk balances node 2
    assert_eq!(
        // node_flk_balance2
        query_runner.get_flk_balance(&owner_secret_key2.to_pk().into()),
        // node_flk_rewards2
        (&emissions_for_node * (&node_2_proportion * HpUfixed::from(4_u64))) / &total_share
    );

    // assert protocols share
    let protocol_account = query_runner.get_protocol_fund_address();
    let protocol_balance = query_runner.get_flk_balance(&protocol_account);
    let protocol_rewards = &emissions * &protocol_share;
    assert_eq!(protocol_balance, protocol_rewards);

    let protocol_stables_balance = query_runner.get_stables_balance(&protocol_account);
    assert_eq!(
        &reward_pool * &protocol_share.convert_precision(),
        protocol_stables_balance
    );

    // assert service balances with service id 0 and 1
    for s in 0..2 {
        let service_owner = query_runner.get_service_info(s).owner;
        let service_balance = query_runner.get_flk_balance(&service_owner);
        assert_eq!(
            service_balance,
            &emissions * &service_share * &service_proportions[s as usize]
        );
        let service_stables_balance = query_runner.get_stables_balance(&service_owner);
        assert_eq!(
            service_stables_balance,
            &reward_pool
                * &service_share.convert_precision()
                * &service_proportions[s as usize].convert_precision()
        );
    }
}

#[tokio::test]
async fn test_get_node_registry() {
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(committee);

    let owner_secret_key1 = AccountOwnerSecretKey::generate();
    let node_secret_key1 = NodeSecretKey::generate();

    // Stake minimum required amount.
    let minimum_stake_amount = query_runner.get_staking_amount().into();
    deposit_and_stake!(
        &update_socket,
        &owner_secret_key1,
        1,
        &minimum_stake_amount,
        &node_secret_key1.to_pk(),
        [0; 96].into()
    );

    // Generate new keys for a different node.
    let owner_secret_key2 = AccountOwnerSecretKey::generate();
    let node_secret_key2 = NodeSecretKey::generate();

    // Stake less than the minimum required amount.
    let less_than_minimum_skate_amount = minimum_stake_amount.clone() / HpUfixed::<18>::from(2u16);
    deposit_and_stake!(
        &update_socket,
        &owner_secret_key2,
        1,
        &less_than_minimum_skate_amount,
        &node_secret_key2.to_pk(),
        [1; 96].into()
    );

    // Generate new keys for a different node.
    let owner_secret_key3 = AccountOwnerSecretKey::generate();
    let node_secret_key3 = NodeSecretKey::generate();

    // Stake minimum required amount.
    deposit!(&update_socket, &owner_secret_key3, 1, &minimum_stake_amount);
    stake!(
        &update_socket,
        &owner_secret_key3,
        2,
        &minimum_stake_amount,
        &node_secret_key3.to_pk(),
        [3; 96].into()
    );

    let valid_nodes = query_runner.get_node_registry(None);
    // We added two valid nodes, so the node registry should contain 2 nodes plus the committee.
    assert_eq!(valid_nodes.len(), 2 + keystore.len());
    assert_valid_node!(&valid_nodes, &query_runner, &node_secret_key1.to_pk());
    // Node registry doesn't contain the invalid node
    assert_not_valid_node!(&valid_nodes, &query_runner, &node_secret_key2.to_pk());
    assert_valid_node!(&valid_nodes, &query_runner, &node_secret_key3.to_pk());

    // We added 3 nodes, so the node registry should contain 3 nodes plus the committee.
    assert_paging_node_registry!(
        &query_runner,
        paging_params(true, 0, keystore.len() + 3),
        3 + keystore.len()
    );
    // We added 2 valid nodes, so the node registry should contain 2 nodes plus the committee.
    assert_paging_node_registry!(
        &query_runner,
        paging_params(false, 0, keystore.len() + 3),
        2 + keystore.len()
    );

    // We get the first 4 nodes.
    assert_paging_node_registry!(
        &query_runner,
        paging_params(true, 0, keystore.len()),
        keystore.len()
    );

    // The first 4 nodes are the committee and we added 3 nodes.
    assert_paging_node_registry!(&query_runner, paging_params(true, 4, keystore.len()), 3);

    // The first 4 nodes are the committee and we added 2 valid nodes.
    assert_paging_node_registry!(
        &query_runner,
        paging_params(false, keystore.len() as u32, keystore.len()),
        2
    );

    // The first 4 nodes are the committee and we added 3 nodes.
    assert_paging_node_registry!(
        &query_runner,
        paging_params(false, keystore.len() as u32, 1),
        1
    );
}

#[tokio::test]
async fn test_supply_across_epoch() {
    let committee_size = 4;
    let (mut committee, mut keystore) = create_genesis_committee(committee_size);

    let epoch_time = 100;
    let max_inflation = 10;
    let protocol_part = 10;
    let node_part = 80;
    let service_part = 10;
    let boost = 4;
    let supply_at_genesis = 1000000;
    let (update_socket, query_runner) = init_app_with_params(
        Params {
            epoch_time: Some(epoch_time),
            max_inflation: Some(max_inflation),
            protocol_share: Some(protocol_part),
            node_share: Some(node_part),
            service_builder_share: Some(service_part),
            max_boost: Some(boost),
            supply_at_genesis: Some(supply_at_genesis),
        },
        Some(committee.clone()),
    );

    // get params for emission calculations
    let percentage_divisor: HpUfixed<18> = 100_u16.into();
    let supply_at_year_start: HpUfixed<18> = supply_at_genesis.into();
    let inflation: HpUfixed<18> = HpUfixed::from(max_inflation) / &percentage_divisor;
    let node_share = HpUfixed::from(node_part) / &percentage_divisor;
    let protocol_share = HpUfixed::from(protocol_part) / &percentage_divisor;
    let service_share = HpUfixed::from(service_part) / &percentage_divisor;

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_secret_key = NodeSecretKey::generate();
    let consensus_secret_key = ConsensusSecretKey::generate();

    let deposit_amount = 10_000_u64.into();
    // deposit FLK tokens and stake it
    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &deposit_amount,
        &node_secret_key.to_pk(),
        consensus_secret_key.to_pk()
    );

    // the index should be increment of whatever the size of genesis committee is, 5 in this case
    add_to_committee(
        &mut committee,
        &mut keystore,
        node_secret_key.clone(),
        consensus_secret_key.clone(),
        owner_secret_key.clone(),
        5,
    );

    // every epoch supply increase similar for simplicity of the test
    let _node_1_usd = 0.1 * 10000_f64;

    // calculate emissions per unit
    let emissions_per_epoch: HpUfixed<18> = (&inflation * &supply_at_year_start) / &365.0.into();

    let mut supply = supply_at_year_start;

    // 365 epoch changes to see if the current supply and year start suppply are ok
    for epoch in 0..365 {
        // add at least one transaction per epoch, so reward pool is not zero
        let nonce = query_runner
            .get_node_info(&node_secret_key.to_pk())
            .unwrap()
            .nonce;
        let pod_10 = prepare_pod_request(10000, 0, &node_secret_key, nonce + 1);
        expect_tx_success!(pod_10, &update_socket);

        // We have to submit uptime measurements to make sure nodes aren't set to
        // participating=false in the next epoch.
        // This is obviously tedious. The alternative is to deactivate the removal of offline nodes
        // for testing.
        for node in &keystore {
            let mut map = BTreeMap::new();
            let measurements = test_reputation_measurements(100);

            for peer in &keystore {
                if node.node_secret_key == peer.node_secret_key {
                    continue;
                }
                let _ = update_reputation_measurements(
                    &query_runner,
                    &mut map,
                    &peer.node_secret_key.to_pk(),
                    measurements.clone(),
                );
            }
            let nonce = query_runner
                .get_node_info(&node.node_secret_key.to_pk())
                .unwrap()
                .nonce
                + 1;

            submit_reputation_measurements!(&update_socket, &node.node_secret_key, nonce, map);
        }

        let (_, new_keystore) = prepare_new_committee(&query_runner, &committee, &keystore);
        simple_epoch_change!(&update_socket, &new_keystore, &query_runner, epoch);

        let supply_increase = &emissions_per_epoch * &node_share
            + &emissions_per_epoch * &protocol_share
            + &emissions_per_epoch * &service_share;
        let total_supply = query_runner.get_total_supply();
        supply += supply_increase;
        assert_eq!(total_supply, supply);
        if epoch == 364 {
            // the supply_year_start should update
            let supply_year_start = query_runner.get_year_start_supply();
            assert_eq!(total_supply, supply_year_start);
        }
    }
}

#[tokio::test]
async fn test_revert_self_transfer() {
    let (update_socket, query_runner) = init_app(None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner: EthAddress = owner_secret_key.to_pk().into();

    let balance = 1_000u64.into();

    deposit!(&update_socket, &owner_secret_key, 1, &balance);
    assert_eq!(query_runner.get_flk_balance(&owner), balance);

    // Check that trying to transfer funds to yourself reverts
    let update = prepare_transfer_request(&10_u64.into(), &owner, &owner_secret_key, 2);
    expect_tx_revert!(update, &update_socket, ExecutionError::CantSendToYourself);

    // Assure that Flk balance has not changed
    assert_eq!(query_runner.get_flk_balance(&owner), balance);
}

#[tokio::test]
async fn test_revert_transfer_not_account_key() {
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(committee);
    let recipient: EthAddress = AccountOwnerSecretKey::generate().to_pk().into();

    let amount: HpUfixed<18> = 10_u64.into();
    let zero_balance = 0u64.into();

    assert_eq!(query_runner.get_flk_balance(&recipient), zero_balance);

    let transfer = UpdateMethod::Transfer {
        amount: amount.clone(),
        token: Tokens::FLK,
        to: recipient,
    };

    // Check that trying to transfer funds with Node Key reverts
    let node_secret_key = &keystore[0].node_secret_key;
    let update_node_key = prepare_update_request_node(transfer.clone(), node_secret_key, 1);
    expect_tx_revert!(
        update_node_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner
    );

    // Check that trying to transfer funds with Consensus Key reverts
    let consensus_secret_key = &keystore[0].consensus_secret_key;
    let update_consensus_key = prepare_update_request_consensus(transfer, consensus_secret_key, 2);
    expect_tx_revert!(
        update_consensus_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner
    );

    // Assure that Flk balance has not changed
    assert_eq!(query_runner.get_flk_balance(&recipient), zero_balance);
}

#[tokio::test]
async fn test_revert_transfer_when_insufficient_balance() {
    let (update_socket, query_runner) = init_app(None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let recipient: EthAddress = AccountOwnerSecretKey::generate().to_pk().into();

    let balance = 10_u64.into();
    let zero_balance = 0u64.into();

    deposit!(&update_socket, &owner_secret_key, 1, &balance);
    assert_eq!(query_runner.get_flk_balance(&recipient), zero_balance);

    // Check that trying to transfer insufficient funds reverts
    let update = prepare_transfer_request(&11u64.into(), &recipient, &owner_secret_key, 2);
    expect_tx_revert!(update, &update_socket, ExecutionError::InsufficientBalance);

    // Assure that Flk balance has not changed
    assert_eq!(query_runner.get_flk_balance(&recipient), zero_balance);
}

#[tokio::test]
async fn test_transfer_works_properly() {
    let (update_socket, query_runner) = init_app(None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner: EthAddress = owner_secret_key.to_pk().into();
    let recipient: EthAddress = AccountOwnerSecretKey::generate().to_pk().into();

    let balance = 1_000u64.into();
    let zero_balance = 0u64.into();
    let transfer_amount: HpUfixed<18> = 10_u64.into();

    deposit!(&update_socket, &owner_secret_key, 1, &balance);

    assert_eq!(query_runner.get_flk_balance(&owner), balance);
    assert_eq!(query_runner.get_flk_balance(&recipient), zero_balance);

    // Check that trying to transfer funds to yourself reverts
    let update = prepare_transfer_request(&10_u64.into(), &recipient, &owner_secret_key, 2);
    expect_tx_success!(update, &update_socket);

    // Assure that Flk balance has decreased for sender
    assert_eq!(
        query_runner.get_flk_balance(&owner),
        balance - transfer_amount.clone()
    );
    // Assure that Flk balance has increased for recipient
    assert_eq!(
        query_runner.get_flk_balance(&recipient),
        zero_balance + transfer_amount
    );
}

#[tokio::test]
async fn test_deposit_flk_works_properly() {
    let (update_socket, query_runner) = init_app(None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner: EthAddress = owner_secret_key.to_pk().into();

    let deposit_amount: HpUfixed<18> = 1_000u64.into();
    let intial_balance = query_runner.get_flk_balance(&owner);

    let deposit = UpdateMethod::Deposit {
        proof: ProofOfConsensus {},
        token: Tokens::FLK,
        amount: deposit_amount.clone(),
    };
    let update = prepare_update_request_account(deposit, &owner_secret_key, 1);
    expect_tx_success!(update, &update_socket);

    assert_eq!(
        query_runner.get_flk_balance(&owner),
        intial_balance + deposit_amount
    );
}

#[tokio::test]
async fn test_revert_deposit_not_account_key() {
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(committee);

    let amount: HpUfixed<18> = 10_u64.into();
    let deposit = UpdateMethod::Deposit {
        proof: ProofOfConsensus {},
        token: Tokens::FLK,
        amount,
    };

    // Check that trying to deposit funds with Node Key reverts
    let node_secret_key = &keystore[0].node_secret_key;
    let update_node_key = prepare_update_request_node(deposit.clone(), node_secret_key, 1);
    expect_tx_revert!(
        update_node_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner
    );

    // Check that trying to deposit funds with Consensus Key reverts
    let consensus_secret_key = &keystore[0].consensus_secret_key;
    let update_consensus_key = prepare_update_request_consensus(deposit, consensus_secret_key, 2);
    expect_tx_revert!(
        update_consensus_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner
    );
}

#[tokio::test]
async fn test_deposit_usdc_works_properly() {
    let (update_socket, query_runner) = init_app(None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner: EthAddress = owner_secret_key.to_pk().into();

    let intial_balance = query_runner.get_account_balance(&owner);
    let deposit_amount = 1_000;
    let deposit = UpdateMethod::Deposit {
        proof: ProofOfConsensus {},
        token: Tokens::USDC,
        amount: deposit_amount.into(),
    };
    let update = prepare_update_request_account(deposit, &owner_secret_key, 1);
    expect_tx_success!(update, &update_socket);

    assert_eq!(
        query_runner.get_account_balance(&owner),
        intial_balance + deposit_amount
    );
}

#[tokio::test]
async fn test_opt_in_reverts_account_key() {
    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(committee);

    // Account Secret Key
    let secret_key = AccountOwnerSecretKey::generate();
    let opt_in = UpdateMethod::OptIn {};
    let update = prepare_update_request_account(opt_in, &secret_key, 1);
    expect_tx_revert!(update, &update_socket, ExecutionError::OnlyNode);
}

#[tokio::test]
async fn test_opt_in_reverts_node_does_not_exist() {
    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(committee);

    // Unknown Node Key (without Stake)
    let node_secret_key = NodeSecretKey::generate();
    let opt_in = UpdateMethod::OptIn {};
    let update = prepare_update_request_node(opt_in, &node_secret_key, 1);
    expect_tx_revert!(update, &update_socket, ExecutionError::NodeDoesNotExist);
}

#[tokio::test]
async fn test_opt_in_reverts_insufficient_stake() {
    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    // New Node key
    let node_secret_key = NodeSecretKey::generate();

    // Stake less than the minimum required amount.
    let minimum_stake_amount: HpUfixed<18> = query_runner.get_staking_amount().into();
    let less_than_minimum_skate_amount: HpUfixed<18> =
        minimum_stake_amount / HpUfixed::<18>::from(2u16);
    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &less_than_minimum_skate_amount,
        &node_secret_key.to_pk(),
        [0; 96].into()
    );

    let opt_in = UpdateMethod::OptIn {};
    let update = prepare_update_request_node(opt_in, &node_secret_key, 1);
    expect_tx_revert!(update, &update_socket, ExecutionError::InsufficientStake);
    assert_ne!(
        query_runner
            .get_node_info(&node_secret_key.to_pk())
            .unwrap()
            .participation,
        Participation::OptedIn
    );
}

#[tokio::test]
async fn test_opt_in_works() {
    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    // New Node
    let node_secret_key = NodeSecretKey::generate();
    let node_pub_key = node_secret_key.to_pk();

    // Stake less than the minimum required amount.
    let minimum_stake_amount: HpUfixed<18> = query_runner.get_staking_amount().into();
    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &minimum_stake_amount,
        &node_pub_key,
        [0; 96].into()
    );

    assert_ne!(
        query_runner
            .get_node_info(&node_pub_key)
            .unwrap()
            .participation,
        Participation::OptedIn
    );

    let opt_in = UpdateMethod::OptIn {};
    let update = prepare_update_request_node(opt_in, &node_secret_key, 1);
    expect_tx_success!(update, &update_socket);

    assert_eq!(
        query_runner
            .get_node_info(&node_pub_key)
            .unwrap()
            .participation,
        Participation::OptedIn
    );
}

#[tokio::test]
async fn test_opt_out_reverts_account_key() {
    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(committee);

    // Account Secret Key
    let secret_key = AccountOwnerSecretKey::generate();
    let opt_out = UpdateMethod::OptOut {};
    let update = prepare_update_request_account(opt_out, &secret_key, 1);
    expect_tx_revert!(update, &update_socket, ExecutionError::OnlyNode);
}

#[tokio::test]
async fn test_opt_out_reverts_node_does_not_exist() {
    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(committee);

    // Unknown Node Key (without Stake)
    let node_secret_key = NodeSecretKey::generate();
    let opt_out = UpdateMethod::OptOut {};
    let update = prepare_update_request_node(opt_out, &node_secret_key, 1);
    expect_tx_revert!(update, &update_socket, ExecutionError::NodeDoesNotExist);
}

#[tokio::test]
async fn test_opt_out_reverts_insufficient_stake() {
    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    // New Node key
    let node_secret_key = NodeSecretKey::generate();

    // Stake less than the minimum required amount.
    let minimum_stake_amount: HpUfixed<18> = query_runner.get_staking_amount().into();
    let less_than_minimum_skate_amount: HpUfixed<18> =
        minimum_stake_amount / HpUfixed::<18>::from(2u16);
    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &less_than_minimum_skate_amount,
        &node_secret_key.to_pk(),
        [0; 96].into()
    );

    let opt_out = UpdateMethod::OptOut {};
    let update = prepare_update_request_node(opt_out, &node_secret_key, 1);
    expect_tx_revert!(update, &update_socket, ExecutionError::InsufficientStake);
    assert_ne!(
        query_runner
            .get_node_info(&node_secret_key.to_pk())
            .unwrap()
            .participation,
        Participation::OptedOut
    );
}

#[tokio::test]
async fn test_opt_out_works() {
    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    // New Node
    let node_secret_key = NodeSecretKey::generate();
    let node_pub_key = node_secret_key.to_pk();

    // Stake less than the minimum required amount.
    let minimum_stake_amount: HpUfixed<18> = query_runner.get_staking_amount().into();
    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &minimum_stake_amount,
        &node_pub_key,
        [0; 96].into()
    );

    assert_ne!(
        query_runner
            .get_node_info(&node_pub_key)
            .unwrap()
            .participation,
        Participation::OptedOut
    );

    let opt_out = UpdateMethod::OptOut {};
    let update = prepare_update_request_node(opt_out, &node_secret_key, 1);
    expect_tx_success!(update, &update_socket);

    assert_eq!(
        query_runner
            .get_node_info(&node_pub_key)
            .unwrap()
            .participation,
        Participation::OptedOut
    );
}

#[tokio::test]
async fn test_revert_stake_not_account_key() {
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(committee);

    let amount: HpUfixed<18> = 1000_u64.into();

    let stake = UpdateMethod::Stake {
        amount,
        node_public_key: keystore[0].node_secret_key.to_pk(),
        consensus_key: None,
        node_domain: None,
        worker_public_key: None,
        worker_domain: None,
        ports: None,
    };

    // Check that trying to Stake funds with Node Key reverts
    let node_secret_key = &keystore[0].node_secret_key;
    let update_node_key = prepare_update_request_node(stake.clone(), node_secret_key, 1);
    expect_tx_revert!(
        update_node_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner
    );

    // Check that trying to Stake funds with Consensus Key reverts
    let consensus_secret_key = &keystore[0].consensus_secret_key;
    let update_consensus_key = prepare_update_request_consensus(stake, consensus_secret_key, 2);
    expect_tx_revert!(
        update_consensus_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner
    );
}

#[tokio::test]
async fn test_revert_stake_insufficient_balance() {
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let address: EthAddress = owner_secret_key.to_pk().into();

    let peer_pub_key = NodeSecretKey::generate().to_pk();

    // Deposit some FLK into an account
    let deposit = 1000_u64.into();
    deposit!(&update_socket, &owner_secret_key, 1, &deposit);

    let balance = query_runner.get_flk_balance(&address);

    // Now try with the correct details for a new node
    let update = prepare_initial_stake_update(
        &(deposit + <u64 as Into<HpUfixed<18>>>::into(1)),
        &peer_pub_key,
        [0; 96].into(),
        "127.0.0.1".parse().unwrap(),
        [0; 32].into(),
        "127.0.0.1".parse().unwrap(),
        NodePorts::default(),
        &owner_secret_key,
        2,
    );

    // Expect Revert Error
    expect_tx_revert!(update, &update_socket, ExecutionError::InsufficientBalance);

    // Flk balance has not changed
    assert_eq!(query_runner.get_flk_balance(&address), balance);
}

#[tokio::test]
async fn test_revert_stake_consensus_key_already_indexed() {
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let address: EthAddress = owner_secret_key.to_pk().into();

    let peer_pub_key = NodeSecretKey::generate().to_pk();

    // Deposit some FLK into an account
    let deposit = 1000_u64.into();
    deposit!(&update_socket, &owner_secret_key, 1, &deposit);

    let balance = query_runner.get_flk_balance(&address);

    // Now try with the correct details for a new node
    let update = prepare_initial_stake_update(
        &deposit,
        &peer_pub_key,
        keystore[0].consensus_secret_key.to_pk(),
        "127.0.0.1".parse().unwrap(),
        [0; 32].into(),
        "127.0.0.1".parse().unwrap(),
        NodePorts::default(),
        &owner_secret_key,
        2,
    );

    // Expect Revert Error
    expect_tx_revert!(
        update,
        &update_socket,
        ExecutionError::ConsensusKeyAlreadyIndexed
    );

    // Flk balance has not changed
    assert_eq!(query_runner.get_flk_balance(&address), balance);
}

#[tokio::test]
async fn test_stake_works() {
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let address: EthAddress = owner_secret_key.to_pk().into();

    let peer_pub_key = NodeSecretKey::generate().to_pk();

    // Deposit some FLK into an account
    let stake = 1000_u64.into();
    deposit!(&update_socket, &owner_secret_key, 1, &stake);

    let balance = query_runner.get_flk_balance(&address);
    let consensus_key: ConsensusPublicKey = [0; 96].into();
    let node_domain: IpAddr = "89.64.54.26".parse().unwrap();
    let worker_pub_key: NodePublicKey = [0; 32].into();
    let worker_domain: IpAddr = "127.0.0.1".parse().unwrap();
    let node_ports = NodePorts {
        primary: 4001,
        worker: 4002,
        mempool: 4003,
        rpc: 4004,
        pool: 4005,
        pinger: 4007,
        handshake: HandshakePorts {
            http: 5001,
            webrtc: 5002,
            webtransport: 5003,
        },
    };
    // Now try with the correct details for a new node
    let update = prepare_initial_stake_update(
        &stake,
        &peer_pub_key,
        consensus_key,
        node_domain,
        worker_pub_key,
        worker_domain,
        node_ports.clone(),
        &owner_secret_key,
        2,
    );

    // Expect Success
    expect_tx_success!(update, &update_socket);

    // Flk balance has not changed
    assert_eq!(
        query_runner.get_flk_balance(&address),
        balance - stake.clone()
    );

    let node_info = query_runner.get_node_info(&peer_pub_key).unwrap();
    assert_eq!(node_info.consensus_key, consensus_key);
    assert_eq!(node_info.domain, node_domain);
    assert_eq!(node_info.worker_public_key, worker_pub_key);
    assert_eq!(node_info.worker_domain, worker_domain);
    assert_eq!(node_info.ports, node_ports);

    // Query the new node and make sure he has the proper stake
    assert_eq!(query_runner.get_staked(&peer_pub_key), stake);

    let node_idx = query_runner.pubkey_to_index(peer_pub_key).unwrap();
    assert_eq!(
        query_runner.index_to_pubkey(node_idx).unwrap(),
        peer_pub_key
    );
}

#[tokio::test]
async fn test_stake_lock_reverts_not_account_key() {
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(committee);

    let stake_lock = UpdateMethod::StakeLock {
        node: NodeSecretKey::generate().to_pk(),
        locked_for: 365,
    };

    // Check that trying to StakeLock funds with Node Key reverts
    let node_secret_key = &keystore[0].node_secret_key;
    let update_node_key = prepare_update_request_node(stake_lock.clone(), node_secret_key, 1);
    expect_tx_revert!(
        update_node_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner
    );

    // Check that trying to StakeLock funds with Consensus Key reverts
    let consensus_secret_key = &keystore[0].consensus_secret_key;
    let update_consensus_key =
        prepare_update_request_consensus(stake_lock, consensus_secret_key, 2);
    expect_tx_revert!(
        update_consensus_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner
    );
}

#[tokio::test]
async fn test_stake_lock_reverts_node_does_not_exist() {
    let (update_socket, _query_runner) = init_app(None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let locked_for = 365;
    let stake_lock_req = prepare_stake_lock_update(&node_pub_key, locked_for, &owner_secret_key, 1);

    expect_tx_revert!(
        stake_lock_req,
        &update_socket,
        ExecutionError::NodeDoesNotExist
    );
}

#[tokio::test]
async fn test_stake_lock_reverts_not_node_owner() {
    let (update_socket, query_runner) = init_app(None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let amount: HpUfixed<18> = 1_000u64.into();

    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &amount,
        &node_pub_key,
        [0; 96].into()
    );

    assert_eq!(query_runner.get_staked(&node_pub_key), amount);

    let locked_for = 365;
    let stake_lock_req = prepare_stake_lock_update(
        &node_pub_key,
        locked_for,
        &AccountOwnerSecretKey::generate(),
        1,
    );

    expect_tx_revert!(stake_lock_req, &update_socket, ExecutionError::NotNodeOwner);
}

#[tokio::test]
async fn test_stake_lock_reverts_insufficient_stake() {
    let (update_socket, query_runner) = init_app(None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let amount: HpUfixed<18> = 0u64.into();

    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &amount,
        &node_pub_key,
        [0; 96].into()
    );

    assert_eq!(query_runner.get_staked(&node_pub_key), amount);

    let locked_for = 365;
    let stake_lock_req = prepare_stake_lock_update(&node_pub_key, locked_for, &owner_secret_key, 3);

    expect_tx_revert!(
        stake_lock_req,
        &update_socket,
        ExecutionError::InsufficientStake
    );
}

#[tokio::test]
async fn test_stake_lock_reverts_lock_exceeded_max_stake_lock_time() {
    let (update_socket, query_runner) = init_app(None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let amount: HpUfixed<18> = 1000u64.into();

    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &amount,
        &node_pub_key,
        [0; 96].into()
    );

    assert_eq!(query_runner.get_staked(&node_pub_key), amount);

    // max locked time from genesis
    let locked_for = 1460 + 1;
    let stake_lock_req = prepare_stake_lock_update(&node_pub_key, locked_for, &owner_secret_key, 3);

    expect_tx_revert!(
        stake_lock_req,
        &update_socket,
        ExecutionError::LockExceededMaxStakeLockTime
    );
}

#[tokio::test]
async fn test_unstake_reverts_not_account_key() {
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(committee);

    let unstake = UpdateMethod::Unstake {
        amount: 100u64.into(),
        node: NodeSecretKey::generate().to_pk(),
    };

    // Check that trying to Unstake funds with Node Key reverts
    let node_secret_key = &keystore[0].node_secret_key;
    let update_node_key = prepare_update_request_node(unstake.clone(), node_secret_key, 1);
    expect_tx_revert!(
        update_node_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner
    );

    // Check that trying to Unstake funds with Consensus Key reverts
    let consensus_secret_key = &keystore[0].consensus_secret_key;
    let update_consensus_key = prepare_update_request_consensus(unstake, consensus_secret_key, 2);
    expect_tx_revert!(
        update_consensus_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner
    );
}

#[tokio::test]
async fn test_unstake_reverts_node_does_not_exist() {
    let (update_socket, _query_runner) = init_app(None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let update = prepare_unstake_update(&100u64.into(), &node_pub_key, &owner_secret_key, 1);

    expect_tx_revert!(update, &update_socket, ExecutionError::NodeDoesNotExist);
}

#[tokio::test]
async fn test_unstake_reverts_insufficient_balance() {
    let (update_socket, query_runner) = init_app(None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let amount: HpUfixed<18> = 1_000u64.into();

    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &amount,
        &node_pub_key,
        [0; 96].into()
    );

    assert_eq!(query_runner.get_staked(&node_pub_key), amount);

    let update = prepare_unstake_update(
        &(amount + <u64 as Into<HpUfixed<18>>>::into(1)),
        &node_pub_key,
        &owner_secret_key,
        3,
    );

    expect_tx_revert!(update, &update_socket, ExecutionError::InsufficientBalance);
}

#[tokio::test]
async fn test_withdraw_unstaked_reverts_not_account_key() {
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(committee);

    let withdraw_unstaked = UpdateMethod::WithdrawUnstaked {
        node: NodeSecretKey::generate().to_pk(),
        recipient: None,
    };

    // Check that trying to Stake funds with Node Key reverts
    let node_secret_key = &keystore[0].node_secret_key;
    let update_node_key =
        prepare_update_request_node(withdraw_unstaked.clone(), node_secret_key, 1);
    expect_tx_revert!(
        update_node_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner
    );

    // Check that trying to Stake funds with Consensus Key reverts
    let consensus_secret_key = &keystore[0].consensus_secret_key;
    let update_consensus_key =
        prepare_update_request_consensus(withdraw_unstaked, consensus_secret_key, 2);
    expect_tx_revert!(
        update_consensus_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner
    );
}

#[tokio::test]
async fn test_withdraw_unstaked_reverts_node_does_not_exist() {
    let (update_socket, _query_runner) = init_app(None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let update = prepare_withdraw_unstaked_update(&node_pub_key, None, &owner_secret_key, 1);

    expect_tx_revert!(update, &update_socket, ExecutionError::NodeDoesNotExist);
}

#[tokio::test]
async fn test_withdraw_unstaked_reverts_not_node_owner() {
    let (update_socket, query_runner) = init_app(None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let amount: HpUfixed<18> = 1_000u64.into();

    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &amount,
        &node_pub_key,
        [0; 96].into()
    );

    assert_eq!(query_runner.get_staked(&node_pub_key), amount);

    let withdraw_unstaked = prepare_withdraw_unstaked_update(
        &node_pub_key,
        None,
        &AccountOwnerSecretKey::generate(),
        1,
    );

    expect_tx_revert!(
        withdraw_unstaked,
        &update_socket,
        ExecutionError::NotNodeOwner
    );
}

#[tokio::test]
async fn test_withdraw_unstaked_reverts_no_locked_tokens() {
    let (update_socket, query_runner) = init_app(None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let amount: HpUfixed<18> = 1_000u64.into();

    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &amount,
        &node_pub_key,
        [0; 96].into()
    );

    assert_eq!(query_runner.get_staked(&node_pub_key), amount);

    let withdraw_unstaked =
        prepare_withdraw_unstaked_update(&node_pub_key, None, &owner_secret_key, 3);

    expect_tx_revert!(
        withdraw_unstaked,
        &update_socket,
        ExecutionError::NoLockedTokens
    );
}

#[tokio::test]
async fn test_withdraw_unstaked_works_properly() {
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner: EthAddress = owner_secret_key.to_pk().into();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let amount: HpUfixed<18> = 1_000u64.into();

    // Stake
    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &amount,
        &node_pub_key,
        [0; 96].into()
    );
    assert_eq!(query_runner.get_staked(&node_pub_key), amount);

    // Unstake
    let update = prepare_unstake_update(&amount, &node_pub_key, &owner_secret_key, 3);
    expect_tx_success!(update, &update_socket);

    // Wait 5 epochs to unlock lock_time (5)
    for epoch in 0..5 {
        simple_epoch_change!(&update_socket, &keystore, &query_runner, epoch);
    }

    let prev_balance = query_runner.get_flk_balance(&owner);

    //Withdraw Unstaked
    let withdraw_unstaked =
        prepare_withdraw_unstaked_update(&node_pub_key, Some(owner), &owner_secret_key, 4);
    expect_tx_success!(withdraw_unstaked, &update_socket);

    // Assert updated Flk balance
    assert_eq!(query_runner.get_flk_balance(&owner), prev_balance + amount);

    // Assert reset the nodes locked stake state
    assert_eq!(
        query_runner
            .get_node_info(&node_pub_key)
            .unwrap()
            .stake
            .locked,
        HpUfixed::zero()
    );
}

#[tokio::test]
async fn test_submit_reputation_measurements_reverts_account_key() {
    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(committee);

    // Account Secret Key
    let secret_key = AccountOwnerSecretKey::generate();
    let opt_in = UpdateMethod::SubmitReputationMeasurements {
        measurements: Default::default(),
    };
    let update = prepare_update_request_account(opt_in, &secret_key, 1);
    expect_tx_revert!(update, &update_socket, ExecutionError::OnlyNode);
}

#[tokio::test]
async fn test_submit_reputation_measurements_reverts_node_does_not_exist() {
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(committee);
    let mut rng = random::get_seedable_rng();

    let mut measurements = BTreeMap::new();
    let _ = update_reputation_measurements(
        &query_runner,
        &mut measurements,
        &keystore[1].node_secret_key.to_pk(),
        reputation::generate_reputation_measurements(&mut rng, 0.1),
    );

    let update = prepare_update_request_node(
        UpdateMethod::SubmitReputationMeasurements { measurements },
        &NodeSecretKey::generate(),
        1,
    );

    expect_tx_revert!(update, &update_socket, ExecutionError::NodeDoesNotExist);
}

#[tokio::test]
async fn test_submit_reputation_measurements_reverts_insufficient_stake() {
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(committee);
    let mut rng = random::get_seedable_rng();

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_secret_key = NodeSecretKey::generate();

    // Stake less than the minimum required amount.
    let minimum_stake_amount: HpUfixed<18> = query_runner.get_staking_amount().into();
    let less_than_minimum_skate_amount: HpUfixed<18> =
        minimum_stake_amount / HpUfixed::<18>::from(2u16);
    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &less_than_minimum_skate_amount,
        &node_secret_key.to_pk(),
        [0; 96].into()
    );

    let mut measurements = BTreeMap::new();
    let _ = update_reputation_measurements(
        &query_runner,
        &mut measurements,
        &keystore[1].node_secret_key.to_pk(),
        reputation::generate_reputation_measurements(&mut rng, 0.1),
    );

    let update = prepare_update_request_node(
        UpdateMethod::SubmitReputationMeasurements { measurements },
        &node_secret_key,
        1,
    );

    expect_tx_revert!(update, &update_socket, ExecutionError::InsufficientStake);
}

#[tokio::test]
async fn test_submit_reputation_measurements_too_many_measurements() {
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(committee);
    let mut rng = random::get_seedable_rng();

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_secret_key = NodeSecretKey::generate();

    // Stake minimum required amount.
    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &query_runner.get_staking_amount().into(),
        &node_secret_key.to_pk(),
        [0; 96].into()
    );

    let mut measurements = BTreeMap::new();

    // create many dummy measurements that len >
    for i in 1..MAX_MEASUREMENTS_PER_TX + 2 {
        measurements.insert(
            i as u32,
            reputation::generate_reputation_measurements(&mut rng, 0.5),
        );
    }
    let update = prepare_update_request_node(
        UpdateMethod::SubmitReputationMeasurements { measurements },
        &node_secret_key,
        1,
    );

    expect_tx_revert!(update, &update_socket, ExecutionError::TooManyMeasurements);
}
