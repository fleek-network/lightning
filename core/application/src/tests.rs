use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::net::IpAddr;
use std::str::FromStr;
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
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    AccountInfo,
    Blake3Hash,
    Block,
    BlockExecutionResponse,
    ChainId,
    CommodityTypes,
    ContentUpdate,
    DeliveryAcknowledgmentProof,
    Epoch,
    ExecutionData,
    ExecutionError,
    HandshakePorts,
    Metadata,
    NodeIndex,
    NodeInfo,
    NodePorts,
    Participation,
    ProofOfConsensus,
    ProtocolParams,
    ReputationMeasurements,
    Staking,
    Tokens,
    TotalServed,
    TransactionRequest,
    TransactionResponse,
    UpdateMethod,
    UpdatePayload,
    UpdateRequest,
    Value,
    MAX_MEASUREMENTS_PER_TX,
    MAX_MEASUREMENTS_SUBMIT,
};
use lightning_interfaces::PagingParams;
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::{random, reputation};
use lightning_utils::application::QueryRunnerExt;
use rand::seq::SliceRandom;
use tempfile::{tempdir, TempDir};

use crate::app::Application;
use crate::config::Config;
use crate::genesis::{Genesis, GenesisAccount, GenesisNode, GenesisPrices, GenesisService};
use crate::state::QueryRunner;

partial!(TestBinding {
    ConfigProviderInterface = JsonConfigProvider;
    ApplicationInterface = Application<Self>;
});

const CHAIN_ID: ChainId = 1337;

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
            let nonce = get_node_nonce($query_runner, &node.node_secret_key.to_pk()) + 1;
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
        let rep_measurements = $query_runner
            .get_reputation_measurements(&$update.0)
            .unwrap();
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
        let node_info = get_node_info($query_runner, $node_pk);
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
        let node_info = get_node_info($query_runner, $node_pk);
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

/// Prepare Genesis Node's Ports
fn test_genesis_ports(index: u16) -> NodePorts {
    let base: u16 = index * 10000;
    NodePorts {
        primary: base + 4310,
        worker: base + 4311,
        mempool: base + 4210,
        rpc: base + 4230,
        pool: base + 4300,
        pinger: base + 4350,
        handshake: HandshakePorts {
            http: base + 4220,
            webrtc: base + 4320,
            webtransport: base + 4321,
        },
    }
}

/// Prepare Test Genesis
fn test_genesis() -> Genesis {
    let genesis_node_owner =
        EthAddress::from_str("0x959807B8D94B324A74117956731F09E2893aCd72").unwrap();
    let domain = "127.0.0.1".parse().unwrap();

    let node_pub_key_1 =
        NodePublicKey::from_str("F5tV4PLSzx1Lt4mYBe13aYQ8hsLMTCfjgY2pLr82AumH").unwrap();
    let consensus_key_1 = ConsensusPublicKey::from_str("u76G7q22Qc5nRC5Fi6dzbNE7FQxqRKEtTS9qjDftWFwhBKmoozGLv8wFiFmGnYDFMEKyYxozWRdM3wgjs1Na3fvxDARxi9CSNJUZJfPXC2WUu3uLnUw96jPBRp7rtHEzS5H").unwrap();
    let node_pub_key_2 =
        NodePublicKey::from_str("Qt1DzUoTEn7n4itYpAPhaDsXhcriuXe1e7n7uZztGfg").unwrap();
    let consensus_key_2 = ConsensusPublicKey::from_str("21cq5icj1pWKk9DBwdUFSW6nqBrwtzHtvKWcLNrqFRxZV8UbdKYzdoSs8C1u7s7M4FKADDqsHnxETh56hdSK2Z65nsbW3xME1fNcT1s8dfHwCFk567mV4fmSSgH73mTe1H3a").unwrap();
    let node_pub_key_3 =
        NodePublicKey::from_str("8XT8Kb1PCd2kzmwHLQ8Nw9aAuKJin6tihaPQBCyf6ymn").unwrap();
    let consensus_key_3 = ConsensusPublicKey::from_str("uNHES9wjYK3HkbcPWrBiQQZ2NmcfBVKke8tbH9X7RFT9ZvjL5f55FfPvpjh2RWpxTRyMhKAxYG42TaRv2RGyEZkYcx2aJMfgPYqYaiT8KC1EPHzJYgVmYc7z2ER69LNWC7r").unwrap();
    let node_pub_key_4 =
        NodePublicKey::from_str("DA3mDUC5y7s5dNF4bL5MfTy7TtXjwt16rtspGuJcwZHS").unwrap();
    let consensus_key_4 = ConsensusPublicKey::from_str("rnSokyL9vj1cnxsrHVmuMCP677Ns4Xh6N5FmfKvjinxVCA9W8w6DiqXSQTX92TtoapS5eqcHCuKnNKamqxh5MnLpHGZ9UkjKUPWsc7hnQXqQobHTXdw1GSh88wEir94mEba").unwrap();

    let test_staking = Staking {
        staked: HpUfixed::<18>::from(1000u32),
        stake_locked_until: 0,
        locked: HpUfixed::<18>::zero(),
        locked_until: 0,
    };

    let genesis_nodes: Vec<GenesisNode> = vec![
        (node_pub_key_1, consensus_key_1),
        (node_pub_key_2, consensus_key_2),
        (node_pub_key_3, consensus_key_3),
        (node_pub_key_4, consensus_key_4),
    ]
    .iter()
    .enumerate()
    .map(|(pos, (pub_key, consensus_key))| {
        GenesisNode::new(
            genesis_node_owner,
            *pub_key,
            domain,
            *consensus_key,
            domain,
            *pub_key,
            test_genesis_ports(pos as u16 + 1),
            Some(test_staking.clone()),
            true,
        )
    })
    .collect();

    let protocol_address =
        EthAddress::from_str("0x2a8cf657769c264b0c7f88e3a716afdeaec1c318").unwrap();

    Genesis {
        chain_id: CHAIN_ID,
        epoch_start: 1684276288383,
        epoch_time: 120000,
        committee_size: 10,
        node_count: 10,
        min_stake: 1000,
        eligibility_time: 1,
        lock_time: 5,
        protocol_share: 0,
        node_share: 80,
        service_builder_share: 20,
        max_inflation: 10,
        consumer_rebate: 0,
        max_boost: 4,
        // 1460 days(epoch) meaning 4 years
        max_lock_time: 1460,
        // Set to 1 million for testing, to be determined when initial allocations are set
        supply_at_genesis: 1000000,
        min_num_measurements: 2,
        protocol_fund_address: protocol_address,
        governance_address: protocol_address,
        node_info: genesis_nodes,
        service: vec![
            GenesisService {
                id: 0,
                owner: EthAddress::from_str("0xDC0A31F9eeb151f82BF1eE6831095284fC215Ee7").unwrap(),
                commodity_type: CommodityTypes::Bandwidth,
            },
            GenesisService {
                id: 1,
                owner: EthAddress::from_str("0x684166BDbf530a256d7c92Fa0a4128669aFd9B9F").unwrap(),
                commodity_type: CommodityTypes::Compute,
            },
        ],
        account: vec![GenesisAccount {
            public_key: genesis_node_owner,
            flk_balance: HpUfixed::<18>::from(100690000000000000000u128),
            stables_balance: 100,
            bandwidth_balance: 100,
        }],
        client: HashMap::new(),
        commodity_prices: vec![
            GenesisPrices {
                commodity: CommodityTypes::Bandwidth,
                price: 0.1,
            },
            GenesisPrices {
                commodity: CommodityTypes::Compute,
                price: 0.2,
            },
        ],
        total_served: HashMap::new(),
        latencies: None,
    }
}

/// Initialize application state with provided or default configuration.
fn init_app(temp_dir: &TempDir, config: Option<Config>) -> (ExecutionEngineSocket, QueryRunner) {
    let config = config.or_else(|| {
        let genesis_path = test_genesis()
            .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
            .unwrap();
        Some(Config::test(genesis_path))
    });
    do_init_app(config.unwrap())
}

/// Initialize application with provided configuration.
fn do_init_app(config: Config) -> (ExecutionEngineSocket, QueryRunner) {
    let node = Node::<TestBinding>::init_with_provider(
        fdi::Provider::default()
            .with(JsonConfigProvider::default().with::<Application<TestBinding>>(config)),
    )
    .expect("failed to initialize node");

    let app = node.provider.get::<Application<TestBinding>>();
    (app.transaction_executor(), app.sync_query())
}

/// Initialize application with provided committee.
fn test_init_app(
    temp_dir: &TempDir,
    committee: Vec<GenesisNode>,
) -> (ExecutionEngineSocket, QueryRunner) {
    let mut genesis = test_genesis();
    genesis.node_info = committee;
    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();
    init_app(temp_dir, Some(Config::test(genesis_path)))
}

/// Initialize application with provided genesis.
fn init_app_with_genesis(
    temp_dir: &TempDir,
    genesis: &Genesis,
) -> (ExecutionEngineSocket, QueryRunner) {
    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();
    init_app(temp_dir, Some(Config::test(genesis_path)))
}

/// Initialize application with provided parameters.
fn init_app_with_params(
    temp_dir: &TempDir,
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

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    init_app(temp_dir, Some(Config::test(genesis_path)))
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
        chain_id: CHAIN_ID,
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
        chain_id: CHAIN_ID,
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
        chain_id: CHAIN_ID,
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
            proofs: vec![DeliveryAcknowledgmentProof],
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

/// Prepare an `UpdateRequest` for `UpdateMethod::UpdateContentRegistry` signed with
/// `NodeSecretKey`. Passing the private key around like this should only be done for testing.
fn prepare_content_registry_update(
    updates: Vec<ContentUpdate>,
    secret_key: &NodeSecretKey,
    nonce: u64,
) -> UpdateRequest {
    prepare_update_request_node(
        UpdateMethod::UpdateContentRegistry { updates },
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
            sub_dag_index: 0,
            sub_dag_round: 0,
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
    let peer_index = get_node_index(query_runner, peer);
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

/// Convert NodePublicKey to NodeIndex
fn get_node_index(query_runner: &QueryRunner, pub_key: &NodePublicKey) -> NodeIndex {
    query_runner.pubkey_to_index(pub_key).unwrap()
}

/// Query NodeTable
fn do_get_node_info<T: Clone>(
    query_runner: &QueryRunner,
    pub_key: &NodePublicKey,
    selector: impl FnOnce(NodeInfo) -> T,
) -> T {
    let node_idx = get_node_index(query_runner, pub_key);
    query_runner
        .get_node_info::<T>(&node_idx, selector)
        .unwrap()
}

/// Query NodeInfo from NodeTable
fn get_node_info(query_runner: &QueryRunner, pub_key: &NodePublicKey) -> NodeInfo {
    do_get_node_info(query_runner, pub_key, |n| n)
}

/// Query Node's Nonce from NodeTable
fn get_node_nonce(query_runner: &QueryRunner, pub_key: &NodePublicKey) -> u64 {
    do_get_node_info::<u64>(query_runner, pub_key, |n| n.nonce)
}

/// Query Node's Participation from NodeTable
fn get_node_participation(query_runner: &QueryRunner, pub_key: &NodePublicKey) -> Participation {
    do_get_node_info::<Participation>(query_runner, pub_key, |n| n.participation)
}

/// Query Node's Stake amount
fn get_staked(query_runner: &QueryRunner, pub_key: &NodePublicKey) -> HpUfixed<18> {
    do_get_node_info::<HpUfixed<18>>(query_runner, pub_key, |n| n.stake.staked)
}

/// Query Node's Locked amount
fn get_locked(query_runner: &QueryRunner, pub_key: &NodePublicKey) -> HpUfixed<18> {
    do_get_node_info::<HpUfixed<18>>(query_runner, pub_key, |n| n.stake.locked)
}

/// Query Node's Locked amount
fn get_locked_time(query_runner: &QueryRunner, pub_key: &NodePublicKey) -> Epoch {
    do_get_node_info::<Epoch>(query_runner, pub_key, |n| n.stake.locked_until)
}

/// Query Node's stake locked until
fn get_stake_locked_until(query_runner: &QueryRunner, pub_key: &NodePublicKey) -> Epoch {
    do_get_node_info::<Epoch>(query_runner, pub_key, |n| n.stake.stake_locked_until)
}

/// Query AccountInfo from AccountTable
fn do_get_account_info<T: Clone>(
    query_runner: &QueryRunner,
    address: &EthAddress,
    selector: impl FnOnce(AccountInfo) -> T,
) -> Option<T> {
    query_runner.get_account_info::<T>(address, selector)
}

/// Query Account's Flk balance
fn get_flk_balance(query_runner: &QueryRunner, address: &EthAddress) -> HpUfixed<18> {
    do_get_account_info::<HpUfixed<18>>(query_runner, address, |a| a.flk_balance)
        .unwrap_or(HpUfixed::<18>::zero())
}

/// Query Account's bandwidth balance
fn get_account_balance(query_runner: &QueryRunner, address: &EthAddress) -> u128 {
    do_get_account_info::<u128>(query_runner, address, |a| a.bandwidth_balance).unwrap_or(0)
}

/// Query Account's stables balance
fn get_stables_balance(query_runner: &QueryRunner, address: &EthAddress) -> HpUfixed<6> {
    do_get_account_info::<HpUfixed<6>>(query_runner, address, |a| a.stables_balance)
        .unwrap_or(HpUfixed::<6>::zero())
}

fn uri_to_providers(query_runner: &QueryRunner, uri: &Blake3Hash) -> Vec<NodeIndex> {
    query_runner
        .get_uri_providers(uri)
        .unwrap_or_default()
        .into_iter()
        .collect()
}

fn content_registry(query_runner: &QueryRunner, node: &NodeIndex) -> Vec<Blake3Hash> {
    query_runner
        .get_content_registry(node)
        .unwrap_or_default()
        .into_iter()
        .collect()
}
//////////////////////////////////////////////////////////////////////////////////
////////////////// This is where the actual tests are defined ////////////////////
//////////////////////////////////////////////////////////////////////////////////

#[tokio::test]
async fn test_genesis_configuration() {
    let temp_dir = tempdir().unwrap();

    // Init application + get the query and update socket
    let (_, query_runner) = init_app(&temp_dir, None);
    // Get the genesis parameters plus the initial committee
    let genesis = test_genesis();
    let genesis_committee = genesis.node_info;
    // For every member of the genesis committee they should have an initial stake of the min stake
    // Query to make sure that holds true
    for node in genesis_committee {
        let balance = get_staked(&query_runner, &node.primary_public_key);
        assert_eq!(HpUfixed::<18>::from(genesis.min_stake), balance);
    }
}

#[tokio::test]
async fn test_epoch_change() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);
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
async fn test_change_epoch_reverts_account_key() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    // Account Secret Key
    let secret_key = AccountOwnerSecretKey::generate();

    let change_epoch = UpdateMethod::ChangeEpoch { epoch: 0 };

    let update = prepare_update_request_account(change_epoch, &secret_key, 1);
    expect_tx_revert!(update, &update_socket, ExecutionError::OnlyNode);
}

#[tokio::test]
async fn test_change_epoch_reverts_node_does_not_exist() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    // Unknown Node Key (without Stake)
    let node_secret_key = NodeSecretKey::generate();
    let change_epoch = UpdateMethod::ChangeEpoch { epoch: 0 };

    let update = prepare_update_request_node(change_epoch, &node_secret_key, 1);
    expect_tx_revert!(update, &update_socket, ExecutionError::NodeDoesNotExist);
}

#[tokio::test]
async fn test_change_epoch_reverts_insufficient_stake() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    // New Node key
    let node_secret_key = NodeSecretKey::generate();

    // Stake less than the minimum required amount.
    let minimum_stake_amount: HpUfixed<18> = query_runner.get_staking_amount().into();
    let less_than_minimum_stake_amount: HpUfixed<18> =
        minimum_stake_amount / HpUfixed::<18>::from(2u16);
    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &less_than_minimum_stake_amount,
        &node_secret_key.to_pk(),
        [0; 96].into()
    );

    let change_epoch = UpdateMethod::ChangeEpoch { epoch: 0 };
    let update = prepare_update_request_node(change_epoch, &node_secret_key, 1);
    expect_tx_revert!(update, &update_socket, ExecutionError::InsufficientStake);
}

#[tokio::test]
async fn test_epoch_change_reverts_epoch_already_changed() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    // call epoch change
    simple_epoch_change!(&update_socket, &keystore, &query_runner, 0);
    assert_eq!(query_runner.get_epoch_info().epoch, 1);

    let change_epoch = UpdateMethod::ChangeEpoch { epoch: 0 };
    let update = prepare_update_request_node(change_epoch, &keystore[0].node_secret_key, 2);
    expect_tx_revert!(update, &update_socket, ExecutionError::EpochAlreadyChanged);
}

#[tokio::test]
async fn test_epoch_change_reverts_epoch_has_not_started() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    let change_epoch = UpdateMethod::ChangeEpoch { epoch: 1 };
    let update = prepare_update_request_node(change_epoch, &keystore[0].node_secret_key, 1);
    expect_tx_revert!(update, &update_socket, ExecutionError::EpochHasNotStarted);
}

#[tokio::test]
async fn test_epoch_change_reverts_not_committee_member() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    // New Node key
    let node_secret_key = NodeSecretKey::generate();

    // Stake less than the minimum required amount.
    let minimum_stake_amount: HpUfixed<18> = query_runner.get_staking_amount().into();

    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &minimum_stake_amount,
        &node_secret_key.to_pk(),
        [0; 96].into()
    );

    let change_epoch = UpdateMethod::ChangeEpoch { epoch: 0 };
    let update = prepare_update_request_node(change_epoch, &node_secret_key, 1);
    expect_tx_revert!(update, &update_socket, ExecutionError::NotCommitteeMember);
}

#[tokio::test]
async fn test_epoch_change_reverts_already_signaled() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    let change_epoch = UpdateMethod::ChangeEpoch { epoch: 0 };
    let update = prepare_update_request_node(change_epoch.clone(), &keystore[0].node_secret_key, 1);
    expect_tx_success!(update, &update_socket);

    // Second update
    let update = prepare_update_request_node(change_epoch, &keystore[0].node_secret_key, 2);
    expect_tx_revert!(update, &update_socket, ExecutionError::AlreadySignaled);
}

#[tokio::test]
async fn test_submit_rep_measurements() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);
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
    let reporting_node_index = get_node_index(&query_runner, &reporting_node_key);

    submit_reputation_measurements!(&update_socket, &keystore[0].node_secret_key, 1, map);

    assert_rep_measurements_update!(&query_runner, update1, reporting_node_index);
    assert_rep_measurements_update!(&query_runner, update2, reporting_node_index);
}

#[tokio::test]
async fn test_submit_rep_measurements_too_many_times() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

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
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);
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

    assert!(query_runner.get_reputation_score(&peer_idx_1).is_some());
    assert!(query_runner.get_reputation_score(&peer_idx_2).is_some());
}

#[tokio::test]
async fn test_uptime_participation() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (mut committee, keystore) = create_genesis_committee(committee_size);
    committee[0].reputation = Some(40);
    committee[1].reputation = Some(80);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let required_signals = calculate_required_signals(committee_size);

    let peer_1 = keystore[2].node_secret_key.to_pk();
    let peer_2 = keystore[3].node_secret_key.to_pk();
    let nonce = 1;

    // Add records in the content registry for all nodes.
    let updates = vec![ContentUpdate {
        uri: [0u8; 32],
        remove: false,
    }];
    let content_registry_update =
        prepare_content_registry_update(updates.clone(), &keystore[2].node_secret_key, 1);
    expect_tx_success!(content_registry_update, &update_socket);
    let content_registry_update =
        prepare_content_registry_update(updates, &keystore[3].node_secret_key, 1);
    expect_tx_success!(content_registry_update, &update_socket);

    // Assert that registries have been updated.
    let index_peer1 = query_runner.pubkey_to_index(&peer_1).unwrap();
    let content_registry1 = content_registry(&query_runner, &index_peer1);
    assert!(!content_registry1.is_empty());

    let index_peer2 = query_runner.pubkey_to_index(&peer_2).unwrap();
    let content_registry2 = content_registry(&query_runner, &index_peer2);
    assert!(!content_registry2.is_empty());

    let providers = uri_to_providers(&query_runner, &[0u8; 32]);
    assert_eq!(providers.len(), 2);

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
    for node in keystore.iter().take(required_signals) {
        change_epoch!(&update_socket, &node.node_secret_key, 2, epoch);
    }

    let node_info1 = get_node_info(&query_runner, &peer_1);
    let node_info2 = get_node_info(&query_runner, &peer_2);

    assert_eq!(node_info1.participation, Participation::False);
    assert_eq!(node_info2.participation, Participation::True);

    // Assert that registries have been updated.
    let content_registry1 = content_registry(&query_runner, &index_peer1);
    assert!(content_registry1.is_empty());

    let content_registry2 = content_registry(&query_runner, &index_peer2);
    assert!(!content_registry2.is_empty());

    let providers = uri_to_providers(&query_runner, &[0u8; 32]);
    assert_eq!(providers.len(), 1);
}

#[tokio::test]
async fn test_stake() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

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
        get_flk_balance(&query_runner, &owner_secret_key.to_pk().into()),
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
    assert_eq!(get_staked(&query_runner, &peer_pub_key), stake_amount);

    // Stake 1000 more but since it is not a new node we should be able to leave the optional
    // parameters out without a revert
    let update = prepare_regular_stake_update(&stake_amount, &peer_pub_key, &owner_secret_key, 5);

    expect_tx_success!(update, &update_socket);

    // Node should now have 2_000 stake
    assert_eq!(
        get_staked(&query_runner, &peer_pub_key),
        (HpUfixed::<18>::from(2u16) * stake_amount.clone())
    );

    // Now test unstake and make sure it moves the tokens to locked status
    let update = prepare_unstake_update(&stake_amount, &peer_pub_key, &owner_secret_key, 6);
    run_update!(update, &update_socket);

    // Check that his locked is 1000 and his remaining stake is 1000
    assert_eq!(get_staked(&query_runner, &peer_pub_key), stake_amount);
    assert_eq!(get_locked(&query_runner, &peer_pub_key), stake_amount);

    // Since this test starts at epoch 0 locked_until will be == lock_time
    assert_eq!(
        get_locked_time(&query_runner, &peer_pub_key),
        test_genesis().lock_time
    );

    // Try to withdraw the locked tokens and it should revert
    let update = prepare_withdraw_unstaked_update(&peer_pub_key, None, &owner_secret_key, 7);

    expect_tx_revert!(update, &update_socket, ExecutionError::TokensLocked);
}

#[tokio::test]
async fn test_stake_lock() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

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

    assert_eq!(get_staked(&query_runner, &node_pub_key), amount);

    let locked_for = 365;
    let stake_lock_req = prepare_stake_lock_update(&node_pub_key, locked_for, &owner_secret_key, 3);

    expect_tx_success!(stake_lock_req, &update_socket);

    assert_eq!(
        get_stake_locked_until(&query_runner, &node_pub_key),
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
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let bandwidth_commodity = 1000;
    let compute_commodity = 2000;
    let bandwidth_pod =
        prepare_pod_request(bandwidth_commodity, 0, &keystore[0].node_secret_key, 1);
    let compute_pod = prepare_pod_request(compute_commodity, 1, &keystore[0].node_secret_key, 2);

    // run the delivery ack transaction
    run_updates!(vec![bandwidth_pod, compute_pod], &update_socket);

    let node_idx = query_runner
        .pubkey_to_index(&keystore[0].node_secret_key.to_pk())
        .unwrap();
    assert_eq!(
        query_runner
            .get_current_epoch_served(&node_idx)
            .unwrap()
            .served,
        vec![bandwidth_commodity, compute_commodity]
    );

    let epoch = 0;

    assert_eq!(
        query_runner.get_total_served(&epoch).unwrap(),
        TotalServed {
            served: vec![bandwidth_commodity, compute_commodity],
            reward_pool: (0.1 * bandwidth_commodity as f64 + 0.2 * compute_commodity as f64).into()
        }
    );
}

#[tokio::test]
async fn test_submit_pod_reverts_account_key() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    // Account Secret Key
    let secret_key = AccountOwnerSecretKey::generate();
    let submit_pod = UpdateMethod::SubmitDeliveryAcknowledgmentAggregation {
        commodity: 2000,
        service_id: 1,
        proofs: vec![DeliveryAcknowledgmentProof],
        metadata: None,
    };
    let update = prepare_update_request_account(submit_pod, &secret_key, 1);
    expect_tx_revert!(update, &update_socket, ExecutionError::OnlyNode);
}

#[tokio::test]
async fn test_submit_pod_reverts_node_does_not_exist() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    // Unknown Node Key (without Stake)
    let node_secret_key = NodeSecretKey::generate();
    let submit_pod = UpdateMethod::SubmitDeliveryAcknowledgmentAggregation {
        commodity: 2000,
        service_id: 1,
        proofs: vec![DeliveryAcknowledgmentProof],
        metadata: None,
    };
    let update = prepare_update_request_node(submit_pod, &node_secret_key, 1);
    expect_tx_revert!(update, &update_socket, ExecutionError::NodeDoesNotExist);
}

#[tokio::test]
async fn test_submit_pod_reverts_insufficient_stake() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    // New Node key
    let node_secret_key = NodeSecretKey::generate();

    // Stake less than the minimum required amount.
    let minimum_stake_amount: HpUfixed<18> = query_runner.get_staking_amount().into();
    let less_than_minimum_stake_amount: HpUfixed<18> =
        minimum_stake_amount / HpUfixed::<18>::from(2u16);
    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &less_than_minimum_stake_amount,
        &node_secret_key.to_pk(),
        [0; 96].into()
    );

    let submit_pod = UpdateMethod::SubmitDeliveryAcknowledgmentAggregation {
        commodity: 2000,
        service_id: 1,
        proofs: vec![DeliveryAcknowledgmentProof],
        metadata: None,
    };
    let update = prepare_update_request_node(submit_pod, &node_secret_key, 1);
    expect_tx_revert!(update, &update_socket, ExecutionError::InsufficientStake);
}

#[tokio::test]
async fn test_submit_pod_reverts_invalid_service_id() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    let update = prepare_pod_request(2000, 1069, &keystore[0].node_secret_key, 1);

    // run the delivery ack transaction
    expect_tx_revert!(update, &update_socket, ExecutionError::InvalidServiceId);
}

#[tokio::test]
async fn test_is_valid_node() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

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
    let less_than_minimum_stake_amount = minimum_stake_amount / HpUfixed::<18>::from(2u16);
    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &less_than_minimum_stake_amount,
        &node_pub_key,
        [1; 96].into()
    );
    // Make sure that this node is not a valid node.
    assert!(!query_runner.is_valid_node(&node_pub_key));
}

#[tokio::test]
async fn test_change_protocol_params() {
    let temp_dir = tempdir().unwrap();

    let governance_secret_key = AccountOwnerSecretKey::generate();
    let governance_public_key = governance_secret_key.to_pk();

    let mut genesis = test_genesis();
    genesis.governance_address = governance_public_key.into();

    let (update_socket, query_runner) = init_app_with_genesis(&temp_dir, &genesis);

    let param = ProtocolParams::LockTime;
    let new_value = 5;
    let update =
        prepare_change_protocol_param_request(&param, &new_value, &governance_secret_key, 1);
    run_update!(update, &update_socket);
    assert_eq!(query_runner.get_protocol_param(&param).unwrap(), new_value);

    let new_value = 8;
    let update =
        prepare_change_protocol_param_request(&param, &new_value, &governance_secret_key, 2);
    run_update!(update, &update_socket);
    assert_eq!(query_runner.get_protocol_param(&param).unwrap(), new_value);

    // Make sure that another private key cannot change protocol parameters.
    let some_secret_key = AccountOwnerSecretKey::generate();
    let minimum_stake_amount = query_runner.get_staking_amount().into();
    deposit!(&update_socket, &some_secret_key, 1, &minimum_stake_amount);

    let malicious_value = 1;
    let update =
        prepare_change_protocol_param_request(&param, &malicious_value, &some_secret_key, 2);
    expect_tx_revert!(update, &update_socket, ExecutionError::OnlyGovernance);
    // Lock time should still be 8.
    assert_eq!(query_runner.get_protocol_param(&param).unwrap(), new_value)
}

#[tokio::test]
async fn test_change_protocol_params_reverts_not_account_key() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let param = ProtocolParams::LockTime;
    let initial_value = query_runner.get_protocol_param(&param).unwrap();
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
        query_runner.get_protocol_param(&param).unwrap(),
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
        query_runner.get_protocol_param(&param).unwrap(),
        initial_value
    );
}

#[tokio::test]
async fn test_simulate_txn() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    // Submit a ChangeEpoch transaction that will revert (EpochHasNotStarted) and ensure that the
    // `simulate_txn` method of the query runner returns the same response as the update runner.
    let invalid_epoch = 1;
    let req = prepare_change_epoch_request(invalid_epoch, &keystore[0].node_secret_key, 1);
    let res = run_update!(req, &update_socket);

    let req = prepare_change_epoch_request(invalid_epoch, &keystore[0].node_secret_key, 2);
    assert_eq!(
        res.txn_receipts[0].response,
        query_runner.simulate_txn(req.into())
    );

    // Submit a ChangeEpoch transaction that will succeed and ensure that the
    // `simulate_txn` method of the query runner returns the same response as the update runner.
    let epoch = 0;
    let req = prepare_change_epoch_request(epoch, &keystore[0].node_secret_key, 2);

    let res = run_update!(req, &update_socket);
    let req = prepare_change_epoch_request(epoch, &keystore[1].node_secret_key, 1);

    assert_eq!(
        res.txn_receipts[0].response,
        query_runner.simulate_txn(req.into())
    );
}

#[tokio::test]
async fn test_distribute_rewards() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);

    let max_inflation = 10;
    let protocol_part = 10;
    let node_part = 80;
    let service_part = 10;
    let boost = 4;
    let supply_at_genesis = 1_000_000;
    let (update_socket, query_runner) = init_app_with_params(
        &temp_dir,
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
        get_stables_balance(&query_runner, &owner_secret_key1.to_pk().into()),
        HpUfixed::<6>::from(node_1_usd) * node_share.convert_precision()
    );
    assert_eq!(
        get_stables_balance(&query_runner, &owner_secret_key2.to_pk().into()),
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
        get_flk_balance(&query_runner, &owner_secret_key1.to_pk().into()),
        // node_flk_rewards1
        (&emissions_for_node * &node_1_proportion) / &total_share
    );

    // assert flk balances node 2
    assert_eq!(
        // node_flk_balance2
        get_flk_balance(&query_runner, &owner_secret_key2.to_pk().into()),
        // node_flk_rewards2
        (&emissions_for_node * (&node_2_proportion * HpUfixed::from(4_u64))) / &total_share
    );

    // assert protocols share
    let protocol_account = match query_runner.get_metadata(&Metadata::ProtocolFundAddress) {
        Some(Value::AccountPublicKey(s)) => s,
        _ => panic!("AccountPublicKey is set genesis and should never be empty"),
    };
    let protocol_balance = get_flk_balance(&query_runner, &protocol_account);
    let protocol_rewards = &emissions * &protocol_share;
    assert_eq!(protocol_balance, protocol_rewards);

    let protocol_stables_balance = get_stables_balance(&query_runner, &protocol_account);
    assert_eq!(
        &reward_pool * &protocol_share.convert_precision(),
        protocol_stables_balance
    );

    // assert service balances with service id 0 and 1
    for s in 0..2 {
        let service_owner = query_runner.get_service_info(&s).unwrap().owner;
        let service_balance = get_flk_balance(&query_runner, &service_owner);
        assert_eq!(
            service_balance,
            &emissions * &service_share * &service_proportions[s as usize]
        );
        let service_stables_balance = get_stables_balance(&query_runner, &service_owner);
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
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

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
    let less_than_minimum_stake_amount = minimum_stake_amount.clone() / HpUfixed::<18>::from(2u16);
    deposit_and_stake!(
        &update_socket,
        &owner_secret_key2,
        1,
        &less_than_minimum_stake_amount,
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

    let valid_nodes = query_runner
        .get_node_registry(None)
        .into_iter()
        .map(|n| n.info)
        .collect::<Vec<NodeInfo>>();
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
    let temp_dir = tempdir().unwrap();

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
        &temp_dir,
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
        let nonce = get_node_nonce(&query_runner, &node_secret_key.to_pk());
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
            let nonce = get_node_nonce(&query_runner, &node.node_secret_key.to_pk()) + 1;

            submit_reputation_measurements!(&update_socket, &node.node_secret_key, nonce, map);
        }

        let (_, new_keystore) = prepare_new_committee(&query_runner, &committee, &keystore);
        simple_epoch_change!(&update_socket, &new_keystore, &query_runner, epoch);

        let supply_increase = &emissions_per_epoch * &node_share
            + &emissions_per_epoch * &protocol_share
            + &emissions_per_epoch * &service_share;

        let total_supply = match query_runner.get_metadata(&Metadata::TotalSupply) {
            Some(Value::HpUfixed(s)) => s,
            _ => panic!("TotalSupply is set genesis and should never be empty"),
        };

        supply += supply_increase;
        assert_eq!(total_supply, supply);

        if epoch == 364 {
            // the supply_year_start should update
            let supply_year_start = match query_runner.get_metadata(&Metadata::SupplyYearStart) {
                Some(Value::HpUfixed(s)) => s,
                _ => panic!("SupplyYearStart is set genesis and should never be empty"),
            };
            assert_eq!(total_supply, supply_year_start);
        }
    }
}

#[tokio::test]
async fn test_revert_self_transfer() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner: EthAddress = owner_secret_key.to_pk().into();

    let balance = 1_000u64.into();

    deposit!(&update_socket, &owner_secret_key, 1, &balance);
    assert_eq!(get_flk_balance(&query_runner, &owner), balance);

    // Check that trying to transfer funds to yourself reverts
    let update = prepare_transfer_request(&10_u64.into(), &owner, &owner_secret_key, 2);
    expect_tx_revert!(update, &update_socket, ExecutionError::CantSendToYourself);

    // Assure that Flk balance has not changed
    assert_eq!(get_flk_balance(&query_runner, &owner), balance);
}

#[tokio::test]
async fn test_revert_transfer_not_account_key() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);
    let recipient: EthAddress = AccountOwnerSecretKey::generate().to_pk().into();

    let amount: HpUfixed<18> = 10_u64.into();
    let zero_balance = 0u64.into();

    assert_eq!(get_flk_balance(&query_runner, &recipient), zero_balance);

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
    assert_eq!(get_flk_balance(&query_runner, &recipient), zero_balance);
}

#[tokio::test]
async fn test_revert_transfer_when_insufficient_balance() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let recipient: EthAddress = AccountOwnerSecretKey::generate().to_pk().into();

    let balance = 10_u64.into();
    let zero_balance = 0u64.into();

    deposit!(&update_socket, &owner_secret_key, 1, &balance);
    assert_eq!(get_flk_balance(&query_runner, &recipient), zero_balance);

    // Check that trying to transfer insufficient funds reverts
    let update = prepare_transfer_request(&11u64.into(), &recipient, &owner_secret_key, 2);
    expect_tx_revert!(update, &update_socket, ExecutionError::InsufficientBalance);

    // Assure that Flk balance has not changed
    assert_eq!(get_flk_balance(&query_runner, &recipient), zero_balance);
}

#[tokio::test]
async fn test_transfer_works_properly() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner: EthAddress = owner_secret_key.to_pk().into();
    let recipient: EthAddress = AccountOwnerSecretKey::generate().to_pk().into();

    let balance = 1_000u64.into();
    let zero_balance = 0u64.into();
    let transfer_amount: HpUfixed<18> = 10_u64.into();

    deposit!(&update_socket, &owner_secret_key, 1, &balance);

    assert_eq!(get_flk_balance(&query_runner, &owner), balance);
    assert_eq!(get_flk_balance(&query_runner, &recipient), zero_balance);

    // Check that trying to transfer funds to yourself reverts
    let update = prepare_transfer_request(&10_u64.into(), &recipient, &owner_secret_key, 2);
    expect_tx_success!(update, &update_socket);

    // Assure that Flk balance has decreased for sender
    assert_eq!(
        get_flk_balance(&query_runner, &owner),
        balance - transfer_amount.clone()
    );
    // Assure that Flk balance has increased for recipient
    assert_eq!(
        get_flk_balance(&query_runner, &recipient),
        zero_balance + transfer_amount
    );
}

#[tokio::test]
async fn test_deposit_flk_works_properly() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner: EthAddress = owner_secret_key.to_pk().into();

    let deposit_amount: HpUfixed<18> = 1_000u64.into();
    let intial_balance = get_flk_balance(&query_runner, &owner);

    let deposit = UpdateMethod::Deposit {
        proof: ProofOfConsensus {},
        token: Tokens::FLK,
        amount: deposit_amount.clone(),
    };
    let update = prepare_update_request_account(deposit, &owner_secret_key, 1);
    expect_tx_success!(update, &update_socket);

    assert_eq!(
        get_flk_balance(&query_runner, &owner),
        intial_balance + deposit_amount
    );
}

#[tokio::test]
async fn test_revert_deposit_not_account_key() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

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
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner: EthAddress = owner_secret_key.to_pk().into();

    let intial_balance = get_account_balance(&query_runner, &owner);
    let deposit_amount = 1_000;
    let deposit = UpdateMethod::Deposit {
        proof: ProofOfConsensus {},
        token: Tokens::USDC,
        amount: deposit_amount.into(),
    };
    let update = prepare_update_request_account(deposit, &owner_secret_key, 1);
    expect_tx_success!(update, &update_socket);

    assert_eq!(
        get_account_balance(&query_runner, &owner),
        intial_balance + deposit_amount
    );
}

#[tokio::test]
async fn test_opt_in_reverts_account_key() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    // Account Secret Key
    let secret_key = AccountOwnerSecretKey::generate();
    let opt_in = UpdateMethod::OptIn {};
    let update = prepare_update_request_account(opt_in, &secret_key, 1);
    expect_tx_revert!(update, &update_socket, ExecutionError::OnlyNode);
}

#[tokio::test]
async fn test_opt_in_reverts_node_does_not_exist() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    // Unknown Node Key (without Stake)
    let node_secret_key = NodeSecretKey::generate();
    let opt_in = UpdateMethod::OptIn {};
    let update = prepare_update_request_node(opt_in, &node_secret_key, 1);
    expect_tx_revert!(update, &update_socket, ExecutionError::NodeDoesNotExist);
}

#[tokio::test]
async fn test_opt_in_reverts_insufficient_stake() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    // New Node key
    let node_secret_key = NodeSecretKey::generate();

    // Stake less than the minimum required amount.
    let minimum_stake_amount: HpUfixed<18> = query_runner.get_staking_amount().into();
    let less_than_minimum_stake_amount: HpUfixed<18> =
        minimum_stake_amount / HpUfixed::<18>::from(2u16);
    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &less_than_minimum_stake_amount,
        &node_secret_key.to_pk(),
        [0; 96].into()
    );

    let opt_in = UpdateMethod::OptIn {};
    let update = prepare_update_request_node(opt_in, &node_secret_key, 1);
    expect_tx_revert!(update, &update_socket, ExecutionError::InsufficientStake);
    assert_ne!(
        get_node_participation(&query_runner, &node_secret_key.to_pk()),
        Participation::OptedIn
    );
}

#[tokio::test]
async fn test_opt_in_works() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

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
        get_node_participation(&query_runner, &node_pub_key),
        Participation::OptedIn
    );

    let opt_in = UpdateMethod::OptIn {};
    let update = prepare_update_request_node(opt_in, &node_secret_key, 1);
    expect_tx_success!(update, &update_socket);

    assert_eq!(
        get_node_participation(&query_runner, &node_pub_key),
        Participation::OptedIn
    );
}

#[tokio::test]
async fn test_opt_out_reverts_account_key() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    // Account Secret Key
    let secret_key = AccountOwnerSecretKey::generate();
    let opt_out = UpdateMethod::OptOut {};
    let update = prepare_update_request_account(opt_out, &secret_key, 1);
    expect_tx_revert!(update, &update_socket, ExecutionError::OnlyNode);
}

#[tokio::test]
async fn test_opt_out_reverts_node_does_not_exist() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    // Unknown Node Key (without Stake)
    let node_secret_key = NodeSecretKey::generate();
    let opt_out = UpdateMethod::OptOut {};
    let update = prepare_update_request_node(opt_out, &node_secret_key, 1);
    expect_tx_revert!(update, &update_socket, ExecutionError::NodeDoesNotExist);
}

#[tokio::test]
async fn test_opt_out_reverts_insufficient_stake() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    // New Node key
    let node_secret_key = NodeSecretKey::generate();

    // Stake less than the minimum required amount.
    let minimum_stake_amount: HpUfixed<18> = query_runner.get_staking_amount().into();
    let less_than_minimum_stake_amount: HpUfixed<18> =
        minimum_stake_amount / HpUfixed::<18>::from(2u16);
    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &less_than_minimum_stake_amount,
        &node_secret_key.to_pk(),
        [0; 96].into()
    );

    let opt_out = UpdateMethod::OptOut {};
    let update = prepare_update_request_node(opt_out, &node_secret_key, 1);
    expect_tx_revert!(update, &update_socket, ExecutionError::InsufficientStake);
    assert_ne!(
        get_node_participation(&query_runner, &node_secret_key.to_pk()),
        Participation::OptedOut
    );
}

#[tokio::test]
async fn test_opt_out_works() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

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
        get_node_participation(&query_runner, &node_pub_key),
        Participation::OptedOut
    );

    let opt_out = UpdateMethod::OptOut {};
    let update = prepare_update_request_node(opt_out, &node_secret_key, 1);
    expect_tx_success!(update, &update_socket);

    assert_eq!(
        get_node_participation(&query_runner, &node_pub_key),
        Participation::OptedOut
    );
}

#[tokio::test]
async fn test_revert_stake_not_account_key() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

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
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let address: EthAddress = owner_secret_key.to_pk().into();

    let peer_pub_key = NodeSecretKey::generate().to_pk();

    // Deposit some FLK into an account
    let deposit = 1000_u64.into();
    deposit!(&update_socket, &owner_secret_key, 1, &deposit);

    let balance = get_flk_balance(&query_runner, &address);

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
    assert_eq!(get_flk_balance(&query_runner, &address), balance);
}

#[tokio::test]
async fn test_revert_stake_consensus_key_already_indexed() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let address: EthAddress = owner_secret_key.to_pk().into();

    let peer_pub_key = NodeSecretKey::generate().to_pk();

    // Deposit some FLK into an account
    let deposit = 1000_u64.into();
    deposit!(&update_socket, &owner_secret_key, 1, &deposit);

    let balance = get_flk_balance(&query_runner, &address);

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
    assert_eq!(get_flk_balance(&query_runner, &address), balance);
}

#[tokio::test]
async fn test_stake_works() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let address: EthAddress = owner_secret_key.to_pk().into();

    let peer_pub_key = NodeSecretKey::generate().to_pk();

    // Deposit some FLK into an account
    let stake = 1000_u64.into();
    deposit!(&update_socket, &owner_secret_key, 1, &stake);

    let balance = get_flk_balance(&query_runner, &address);
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
        get_flk_balance(&query_runner, &address),
        balance - stake.clone()
    );

    let node_info = get_node_info(&query_runner, &peer_pub_key);
    assert_eq!(node_info.consensus_key, consensus_key);
    assert_eq!(node_info.domain, node_domain);
    assert_eq!(node_info.worker_public_key, worker_pub_key);
    assert_eq!(node_info.worker_domain, worker_domain);
    assert_eq!(node_info.ports, node_ports);

    // Query the new node and make sure he has the proper stake
    assert_eq!(get_staked(&query_runner, &peer_pub_key), stake);

    let node_idx = query_runner.pubkey_to_index(&peer_pub_key).unwrap();
    assert_eq!(
        query_runner.index_to_pubkey(&node_idx).unwrap(),
        peer_pub_key
    );
}

#[tokio::test]
async fn test_stake_lock_reverts_not_account_key() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

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
    let temp_dir = tempdir().unwrap();

    let (update_socket, _query_runner) = init_app(&temp_dir, None);

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
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

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

    assert_eq!(get_staked(&query_runner, &node_pub_key), amount);

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
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

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

    assert_eq!(get_staked(&query_runner, &node_pub_key), amount);

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
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

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

    assert_eq!(get_staked(&query_runner, &node_pub_key), amount);

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
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

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
    let temp_dir = tempdir().unwrap();

    let (update_socket, _query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let update = prepare_unstake_update(&100u64.into(), &node_pub_key, &owner_secret_key, 1);

    expect_tx_revert!(update, &update_socket, ExecutionError::NodeDoesNotExist);
}

#[tokio::test]
async fn test_unstake_reverts_insufficient_balance() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

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

    assert_eq!(get_staked(&query_runner, &node_pub_key), amount);

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
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

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
    let temp_dir = tempdir().unwrap();

    let (update_socket, _query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let update = prepare_withdraw_unstaked_update(&node_pub_key, None, &owner_secret_key, 1);

    expect_tx_revert!(update, &update_socket, ExecutionError::NodeDoesNotExist);
}

#[tokio::test]
async fn test_withdraw_unstaked_reverts_not_node_owner() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

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

    assert_eq!(get_staked(&query_runner, &node_pub_key), amount);

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
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

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

    assert_eq!(get_staked(&query_runner, &node_pub_key), amount);

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
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

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
    assert_eq!(get_staked(&query_runner, &node_pub_key), amount);

    // Unstake
    let update = prepare_unstake_update(&amount, &node_pub_key, &owner_secret_key, 3);
    expect_tx_success!(update, &update_socket);

    // Wait 5 epochs to unlock lock_time (5)
    for epoch in 0..5 {
        simple_epoch_change!(&update_socket, &keystore, &query_runner, epoch);
    }

    let prev_balance = get_flk_balance(&query_runner, &owner);

    //Withdraw Unstaked
    let withdraw_unstaked =
        prepare_withdraw_unstaked_update(&node_pub_key, Some(owner), &owner_secret_key, 4);
    expect_tx_success!(withdraw_unstaked, &update_socket);

    // Assert updated Flk balance
    assert_eq!(
        get_flk_balance(&query_runner, &owner),
        prev_balance + amount
    );

    // Assert reset the nodes locked stake state
    assert_eq!(
        query_runner
            .get_node_info::<HpUfixed<18>>(
                &query_runner.pubkey_to_index(&node_pub_key).unwrap(),
                |n| n.stake.locked
            )
            .unwrap(),
        HpUfixed::zero()
    );
}

#[tokio::test]
async fn test_submit_reputation_measurements_reverts_account_key() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

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
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);
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
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);
    let mut rng = random::get_seedable_rng();

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_secret_key = NodeSecretKey::generate();

    // Stake less than the minimum required amount.
    let minimum_stake_amount: HpUfixed<18> = query_runner.get_staking_amount().into();
    let less_than_minimum_stake_amount: HpUfixed<18> =
        minimum_stake_amount / HpUfixed::<18>::from(2u16);
    deposit_and_stake!(
        &update_socket,
        &owner_secret_key,
        1,
        &less_than_minimum_stake_amount,
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
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);
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

#[tokio::test]
async fn test_submit_content_registry_update() {
    let temp_dir = tempdir().unwrap();

    // Given: committee and setup.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    // When: each node sends an update to register some content.
    let mut expected_records = Vec::new();
    for (list_idx, ck) in keystore.iter().enumerate() {
        let uri = [list_idx as u8; 32];
        expected_records.push((ck.node_secret_key.clone(), uri));

        let updates = vec![ContentUpdate { uri, remove: false }];
        let update = prepare_content_registry_update(updates, &ck.node_secret_key, 1);
        expect_tx_success!(update, &update_socket);
    }

    // Then: registry indicates that the nodes are providing the correct content.
    for (sk, cid) in expected_records.iter() {
        let index = query_runner.pubkey_to_index(&sk.to_pk()).unwrap();
        let cids = content_registry(&query_runner, &index);
        assert_eq!(cids, vec![*cid]);
    }
    // Then: all providers are accounted for.
    for (sk, cid) in expected_records.iter() {
        let index = query_runner.pubkey_to_index(&sk.to_pk()).unwrap();
        let providers = uri_to_providers(&query_runner, cid);
        assert_eq!(providers, vec![index]);
    }

    // When: send an update to remove the records.
    for (sk, uri) in expected_records.iter() {
        let updates = vec![ContentUpdate {
            uri: *uri,
            remove: true,
        }];
        let update = prepare_content_registry_update(updates, sk, 2);
        expect_tx_success!(update, &update_socket);
    }

    // Then: records are removed.
    for (sk, uri) in expected_records.iter() {
        let providers = uri_to_providers(&query_runner, uri);
        assert!(providers.is_empty());
        let index = query_runner.pubkey_to_index(&sk.to_pk()).unwrap();
        let cids = content_registry(&query_runner, &index);
        assert!(cids.is_empty());
    }
}

#[tokio::test]
async fn test_submit_content_registry_update_multiple_providers_per_cid() {
    let temp_dir = tempdir().unwrap();

    // Given: committee and setup.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    // Given: a cid.
    let uri = [69u8; 32];
    // Given: providers for that cid.
    let providers = keystore.clone();

    // When: all nodes send an update for that cid.
    for ck in &providers {
        let updates = vec![ContentUpdate { uri, remove: false }];
        let update = prepare_content_registry_update(updates, &ck.node_secret_key, 1);
        expect_tx_success!(update, &update_socket);
    }

    // Then: state shows that they all provide the cid.
    let expected_providers = providers
        .into_iter()
        .map(|ck| {
            query_runner
                .pubkey_to_index(&ck.node_secret_key.to_pk())
                .unwrap()
        })
        .collect::<Vec<_>>();
    let providers = uri_to_providers(&query_runner, &uri);
    assert_eq!(providers, expected_providers);
    // Then: cid is in every node's registry.
    for provider in providers {
        let uris = content_registry(&query_runner, &provider);
        assert_eq!(vec![uri], uris);
    }

    // When: send an update to remove the records.
    let providers = keystore.clone();
    for ck in &providers {
        let updates = vec![ContentUpdate { uri, remove: true }];
        let update = prepare_content_registry_update(updates, &ck.node_secret_key, 2);
        expect_tx_success!(update, &update_socket);
    }

    // Then: records are removed.
    let providers = uri_to_providers(&query_runner, &uri);
    assert!(providers.is_empty());
    for node in keystore {
        let index = query_runner
            .pubkey_to_index(&node.node_secret_key.to_pk())
            .unwrap();
        let cids = content_registry(&query_runner, &index);
        assert!(cids.is_empty());
    }
}

#[tokio::test]
async fn test_submit_content_registry_update_mix_of_add_and_remove_updates() {
    let temp_dir = tempdir().unwrap();

    // Given: committee and setup.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    // Given: multiple cids.
    let mut uris = Vec::new();
    for i in 0..6 {
        let uri = [i as u8; 32];
        uris.push(uri);
    }

    // Given: a node provides the given cids.
    let mut updates = Vec::new();
    for uri in &uris {
        updates.push(ContentUpdate {
            uri: *uri,
            remove: false,
        });
    }

    let update = prepare_content_registry_update(updates.clone(), &keystore[0].node_secret_key, 1);
    expect_tx_success!(update, &update_socket);

    // When: we remove some records and add some new ones.
    let mut removed = Vec::new();
    let mut updates = updates
        .into_iter()
        .step_by(2)
        .map(|mut update| {
            update.remove = true;
            removed.push(update.uri);
            update
        })
        .collect::<Vec<_>>();

    for i in 6..9 {
        let uri = [i as u8; 32];
        uris.push(uri);
        updates.push(ContentUpdate { uri, remove: false });
    }

    let update = prepare_content_registry_update(updates, &keystore[0].node_secret_key, 2);
    expect_tx_success!(update, &update_socket);

    // Then: the state is updated appropriately.
    // Check that removed is a subset so that both branches below can be asserted.
    assert!(!removed.is_empty());
    assert!(removed.len() < uris.len());
    let node = query_runner
        .pubkey_to_index(&keystore[0].node_secret_key.to_pk())
        .unwrap();
    for uri in &uris {
        if removed.contains(uri) {
            let providers = uri_to_providers(&query_runner, uri);
            assert!(providers.is_empty());
        } else {
            let providers = uri_to_providers(&query_runner, uri);
            assert_eq!(providers, vec![node]);
        }
    }
    let expected_uris = uris
        .iter()
        .copied()
        .filter(|uri| !removed.contains(uri))
        .collect::<Vec<_>>();
    let uris_for_node = content_registry(&query_runner, &node);
    assert_eq!(expected_uris, uris_for_node);
}

#[tokio::test]
async fn test_submit_content_registry_update_too_many_updates_in_transaction() {
    let temp_dir = tempdir().unwrap();

    // Given: committee and setup.
    let committee_size = 2;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _) = test_init_app(&temp_dir, committee);

    // Given: a big list of updates.
    let uri = [69u8; 32];
    let mut updates = Vec::new();
    for _ in 0..101u32 {
        updates.push(ContentUpdate { uri, remove: false });
    }

    // When: we submit the update transaction.
    let update = prepare_content_registry_update(updates, &keystore[0].node_secret_key, 1);

    // Then: the transaction reverts because the list went past the limit.
    expect_tx_revert!(update, &update_socket, ExecutionError::TooManyUpdates);
}

#[tokio::test]
async fn test_submit_content_registry_update_multiple_cids_per_provider() {
    let temp_dir = tempdir().unwrap();

    // Given: committee and setup.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    // Given: multiple cids that some nodes will provide.
    let mut uris = Vec::new();
    let mut updates = Vec::new();
    for i in 0..6 {
        let uri = [i as u8; 32];
        uris.push(uri);
        updates.push(ContentUpdate { uri, remove: false });
    }

    // When: each node sends an update to register the cids.
    let update = prepare_content_registry_update(updates.clone(), &keystore[0].node_secret_key, 1);
    expect_tx_success!(update, &update_socket);

    let update = prepare_content_registry_update(updates, &keystore[1].node_secret_key, 1);
    expect_tx_success!(update, &update_socket);

    // Then: nodes show up as providers for the cids.
    let node1 = query_runner
        .pubkey_to_index(&keystore[0].node_secret_key.to_pk())
        .unwrap();
    let node2 = query_runner
        .pubkey_to_index(&keystore[1].node_secret_key.to_pk())
        .unwrap();

    for uri in &uris {
        let providers = uri_to_providers(&query_runner, uri);
        assert_eq!(providers, vec![node1, node2]);
    }
    // Then: each node is providing the correct set of cids.
    let uris1 = content_registry(&query_runner, &node1);
    assert_eq!(uris1, uris);
    let uris2 = content_registry(&query_runner, &node2);
    assert_eq!(uris2, uris);

    // When: one of the nodes submits an update to remove a record for one cid.
    let updates = vec![ContentUpdate {
        uri: uris[0],
        remove: true,
    }];
    let update = prepare_content_registry_update(updates, &keystore[0].node_secret_key, 2);
    expect_tx_success!(update, &update_socket);

    // Then: the record is removed for that one provider.
    let providers = uri_to_providers(&query_runner, &uris[0]);
    assert_eq!(providers, vec![node2]);
    // Then: the rest of the records are not affected.
    for uri in uris.iter().skip(1) {
        let providers = uri_to_providers(&query_runner, uri);
        assert_eq!(providers, vec![node1, node2]);
    }
    let expected_cids = uris.clone()[1..].to_vec();
    let cids1 = content_registry(&query_runner, &node1);
    assert_eq!(cids1, expected_cids);

    // When: we remove the rest for that same node.
    let mut updates = Vec::new();
    for uri in uris.iter().skip(1) {
        updates.push(ContentUpdate {
            uri: *uri,
            remove: true,
        })
    }
    let update = prepare_content_registry_update(updates, &keystore[0].node_secret_key, 3);
    expect_tx_success!(update, &update_socket);

    // Then: all records for that provider are gone.
    for uri in uris {
        let providers = uri_to_providers(&query_runner, &uri);
        assert_eq!(providers, vec![node2]);
    }
    let cids = content_registry(&query_runner, &node1);
    assert!(cids.is_empty());
}

#[tokio::test]
async fn test_submit_content_registry_update_multiple_updates_for_cid_in_same_transaction() {
    let temp_dir = tempdir().unwrap();

    // Given: committee and setup.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _) = test_init_app(&temp_dir, committee);

    // Given: a cid.
    let uri = [0u8; 32];

    // When: Add and remove the same given cid in the same transaction for some node.
    let updates = vec![
        ContentUpdate { uri, remove: false },
        ContentUpdate { uri, remove: true },
    ];
    let update = prepare_content_registry_update(updates, &keystore[0].node_secret_key, 1);

    // Then: the transaction is reverted.
    expect_tx_revert!(
        update,
        &update_socket,
        ExecutionError::TooManyUpdatesForContent
    );

    // When: we provide same updates for cid in the same transaction.
    let uri = [88u8; 32];
    let updates = vec![
        ContentUpdate { uri, remove: false },
        ContentUpdate { uri, remove: false },
    ];
    let update = prepare_content_registry_update(updates, &keystore[0].node_secret_key, 2);

    // Then: the transaction is reverted.
    expect_tx_revert!(
        update,
        &update_socket,
        ExecutionError::TooManyUpdatesForContent
    );
}

#[tokio::test]
async fn test_submit_content_registry_update_multiple_updates_for_cid_in_diff_transactions() {
    let temp_dir = tempdir().unwrap();

    // Given: committee and setup.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _) = test_init_app(&temp_dir, committee);

    // Given: a cid.
    let uri = [0u8; 32];
    // Given: register given cid in the node's register.
    let updates = vec![ContentUpdate { uri, remove: false }];
    let update = prepare_content_registry_update(updates, &keystore[0].node_secret_key, 1);
    expect_tx_success!(update, &update_socket);

    // When: we submit same update for same cid in different transaction.
    let updates = vec![ContentUpdate { uri, remove: false }];
    let update = prepare_content_registry_update(updates, &keystore[0].node_secret_key, 2);

    // Then: the transaction is successful.
    expect_tx_success!(update, &update_socket);
}

#[tokio::test]
async fn test_submit_content_registry_update_remove_unknown_cid() {
    let temp_dir = tempdir().unwrap();

    // Given: committee and setup.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _) = test_init_app(&temp_dir, committee);

    // Given: a cid.
    let uri = [0u8; 32];
    let updates = vec![ContentUpdate { uri, remove: false }];

    // Given: one node provides the cid.
    let update = prepare_content_registry_update(updates.clone(), &keystore[1].node_secret_key, 1);
    expect_tx_success!(update, &update_socket);

    // Given: another node provides other cids except the given cid.
    let mut updates = Vec::new();
    for idx in 1..4u8 {
        updates.push(ContentUpdate {
            uri: [idx; 32],
            remove: false,
        });
    }
    let update = prepare_content_registry_update(updates, &keystore[0].node_secret_key, 1);
    expect_tx_success!(update, &update_socket);

    // When: that node tries to remove registry for the given cid which it doesn't have.
    let updates = vec![ContentUpdate { uri, remove: true }];
    let update = prepare_content_registry_update(updates, &keystore[0].node_secret_key, 2);

    // Then: the transaction is reverted.
    expect_tx_revert!(
        update,
        &update_socket,
        ExecutionError::InvalidContentRemoval
    )
}

#[tokio::test]
async fn test_submit_content_registry_update_remove_unknown_cid_empty_registry() {
    let temp_dir = tempdir().unwrap();

    // Given: committee and setup.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _) = test_init_app(&temp_dir, committee);

    // Given: some cid that is not in the registry and the node is not providing any content.
    let updates = vec![ContentUpdate {
        uri: [68u8; 32],
        remove: true,
    }];

    // When: we try to remove the cid from the registry.
    let update = prepare_content_registry_update(updates, &keystore[0].node_secret_key, 1);

    // Then: the transaction is reverted.
    expect_tx_revert!(
        update,
        &update_socket,
        ExecutionError::InvalidStateForContentRemoval
    );
}

#[tokio::test]
async fn test_invalid_chain_id() {
    let temp_dir = tempdir().unwrap();

    let chain_id = CHAIN_ID + 1;
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    // Submit a OptIn transaction that will revert (InvalidChainID).

    // Regular Txn Execution
    let secret_key = &keystore[0].node_secret_key;
    let payload = UpdatePayload {
        sender: secret_key.to_pk().into(),
        nonce: 1,
        method: UpdateMethod::OptIn {},
        chain_id,
    };
    let digest = payload.to_digest();
    let signature = secret_key.sign(&digest);
    let update = UpdateRequest {
        signature: signature.into(),
        payload: payload.clone(),
    };
    expect_tx_revert!(update, &update_socket, ExecutionError::InvalidChainId);
}

// (dalton) Since the quick sort used to select the winners of the auctions takes &self of the whole
// state, since it has to do reputation lookups on the compare nodes side of things I am going to
// repeate the modified quick sort algorithm here so we can have unit tests on just the actual
// algoritm
#[test]
fn test_quick_sort() {
    let nodes = quick_sort_mock_node_list();
    // We want the top 1000 nodes so the algorithm will find us and return the pivot from the bottom
    // 9000
    let k = 9000;
    let r = nodes.len() - 1;
    let winners = quick_sort_repeated(nodes, 0, r, k);

    assert_eq!(winners.len(), 1000);

    for node in winners {
        // Node indexes 9000-10000 should be the winners of the auction in this test
        assert!(node.0 > 8999);
    }
}

fn quick_sort_repeated(
    mut nodes: Vec<(NodeIndex, NodeInfo)>,
    l: usize,
    r: usize,
    k: usize,
) -> Vec<(NodeIndex, NodeInfo)> {
    let pivot = quick_sort_partition_repeated(&mut nodes, l, r);

    match pivot.cmp(&(k - 1)) {
        Ordering::Equal => nodes[pivot + 1..].to_vec(),
        Ordering::Greater => quick_sort_repeated(nodes, l, pivot - 1, k),
        _ => quick_sort_repeated(nodes, pivot + 1, r, k),
    }
}

fn quick_sort_partition_repeated(nodes: &mut [(NodeIndex, NodeInfo)], l: usize, r: usize) -> usize {
    let pivot = nodes[r].clone();
    let mut i = l;

    for j in l..r {
        if compare_nodes_repeated(&nodes[j].1, &pivot.1) {
            nodes.swap(j, i);
            i += 1;
        }
    }
    nodes.swap(i, r);

    i
}

fn compare_nodes_repeated(left: &NodeInfo, right: &NodeInfo) -> bool {
    left.stake.staked <= right.stake.staked
}

// Provides list of 10k nodes each one staking one more than the last
fn quick_sort_mock_node_list() -> Vec<(NodeIndex, NodeInfo)> {
    let mut nodes = Vec::with_capacity(10_000);
    for i in 0..10_000 {
        nodes.push((
            i,
            NodeInfo {
                owner: [0; 20].into(),
                public_key: [0; 32].into(),
                consensus_key: [0; 96].into(),
                staked_since: 0,
                stake: Staking {
                    staked: i.into(),
                    stake_locked_until: 0,
                    locked: HpUfixed::zero(),
                    locked_until: 0,
                },
                domain: [0, 0, 0, 0].into(),
                worker_domain: [0, 0, 0, 0].into(),
                worker_public_key: [0; 32].into(),
                participation: Participation::True,
                nonce: 0,
                ports: Default::default(),
            },
        ));
    }
    nodes.shuffle(&mut rand::thread_rng());
    nodes
}
