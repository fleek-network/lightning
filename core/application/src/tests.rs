use std::{
    collections::BTreeMap,
    time::{Duration, SystemTime},
    vec,
};

use affair::Socket;
use anyhow::{anyhow, Result};
use big_decimal::BigDecimal;
use draco_interfaces::{
    application::ExecutionEngineSocket,
    types::{
        Block, Epoch, ExecutionError, NodeInfo, ProofOfConsensus, ProtocolParams,
        ReputationMeasurements, Tokens, TotalServed, TransactionResponse, UpdateMethod,
        UpdatePayload, UpdateRequest,
    },
    ApplicationInterface, BlockExecutionResponse, DeliveryAcknowledgment, SyncQueryRunnerInterface,
};
use fastcrypto::{
    bls12381::min_sig::BLS12381PublicKey, ed25519::Ed25519PublicKey, traits::EncodeDecodeBase64,
};
use fleek_crypto::{
    AccountOwnerPublicKey, AccountOwnerSignature, NodePublicKey, NodeSignature,
    TransactionSignature,
};
use rand::{rngs::StdRng, Rng, SeedableRng};
use tokio::test;

use crate::{
    app::Application,
    config::{Config, Mode},
    genesis::Genesis,
    query_runner::QueryRunner,
};

pub struct Params {
    epoch_time: Option<String>,
    max_inflation: Option<String>,
    protocol_share: Option<String>,
    node_share: Option<String>,
    validator_share: Option<String>,
    max_boost: Option<String>,
    supply_at_genesis: Option<String>,
}

const ACCOUNT_ONE: AccountOwnerPublicKey = AccountOwnerPublicKey([0; 32]);
const NODE_ONE: &str = "k7XAk/1z4rXf1QHyMPHZ1cgyeX2T3bsCCopNpFV6v8hInZfjyti79w3raEa3YwFADM2BnX+/o49k1HQjKZIYlGDszEZ/zUaK3kn3MfT5BEWkKgP+TFMPJoBxenV33XEZ";

// Init the app and return the execution engine socket that would go to narwhal and the query socket
// that could go to anyone
async fn init_app(config: Option<Config>) -> (ExecutionEngineSocket, QueryRunner) {
    let config = config.or_else(|| Some(Config::default()));
    let app = Application::init(config.unwrap()).await.unwrap();

    (app.transaction_executor(), app.sync_query())
}

async fn init_app_with_params(params: Params) -> (ExecutionEngineSocket, QueryRunner) {
    let mut genesis = Genesis::load().expect("Failed to load genesis from file.");

    genesis.epoch_start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .to_string();
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

    if let Some(validator_share) = params.validator_share {
        genesis.validator_share = validator_share;
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
    };

    init_app(Some(config)).await
}

fn node_and_account_key(value: u8) -> (AccountOwnerPublicKey, NodePublicKey) {
    (
        AccountOwnerPublicKey([value; 32]),
        NodePublicKey([value; 96]),
    )
}

async fn simple_epoch_change(
    epoch: Epoch,
    committee_members: &Vec<NodePublicKey>,
    update_socket: &Socket<Block, BlockExecutionResponse>,
) -> Result<()> {
    let required_signals = 2 * committee_members.len() / 3 + 1;
    // make call epoch change for 2/3rd committe members
    for (index, node) in committee_members.iter().enumerate().take(required_signals) {
        let req = get_update_request_node(UpdateMethod::ChangeEpoch { epoch }, *node);
        let res = run_transaction(vec![req], update_socket).await?;
        // check epoch change

        if index == required_signals - 1 {
            assert!(res.change_epoch);
        }
    }
    Ok(())
}

// Helper method to get a transaction update request from a node
// This is just putting a default signature+ nonce so will need to be updated
// when transaction verification is implemented
fn get_update_request_node(method: UpdateMethod, sender: NodePublicKey) -> UpdateRequest {
    UpdateRequest {
        sender: sender.into(),
        signature: TransactionSignature::Node(NodeSignature([0; 48])),
        payload: UpdatePayload { nonce: 0, method },
    }
}

fn get_update_request_account(
    method: UpdateMethod,
    sender: AccountOwnerPublicKey,
) -> UpdateRequest {
    UpdateRequest {
        sender: sender.into(),
        signature: TransactionSignature::AccountOwner(AccountOwnerSignature),
        payload: UpdatePayload { nonce: 0, method },
    }
}

fn get_genesis() -> (Genesis, Vec<NodeInfo>) {
    let genesis = Genesis::load().unwrap();

    (
        genesis.clone(),
        genesis.committee.iter().map(|node| node.into()).collect(),
    )
}
// Helper methods for tests
fn pod_request(node: NodePublicKey, commodity: u128, service_id: u32) -> UpdateRequest {
    get_update_request_node(
        UpdateMethod::SubmitDeliveryAcknowledgmentAggregation {
            commodity,  // units of data served
            service_id, // service 0 serving bandwidth
            proofs: vec![DeliveryAcknowledgment::default()],
            metadata: None,
        },
        node,
    )
}

fn get_seedable_rng() -> StdRng {
    let seed: [u8; 32] = (0..32).collect::<Vec<u8>>().try_into().unwrap();
    SeedableRng::from_seed(seed)
}

fn generate_reputation_measurements(
    rng: &mut StdRng,
    prob_measurement_present: f64,
) -> ReputationMeasurements {
    let latency = if rng.gen_bool(prob_measurement_present) {
        None
    } else {
        Some(Duration::from_millis(rng.gen_range(100..=400)))
    };
    let interactions = if rng.gen_bool(prob_measurement_present) {
        None
    } else {
        Some(rng.gen_range(-20..=100))
    };
    let inbound_bandwidth = if rng.gen_bool(prob_measurement_present) {
        None
    } else {
        // bytes per milliseconds: 50 Mbps to 250 Mbps
        Some(rng.gen_range(6250..31250))
    };
    let outbound_bandwidth = if rng.gen_bool(prob_measurement_present) {
        None
    } else {
        // bytes per milliseconds: 50 Mbps to 250 Mbps
        Some(rng.gen_range(6250..31250))
    };
    let bytes_received = if rng.gen_bool(prob_measurement_present) {
        None
    } else {
        Some(rng.gen_range(100_000..1_000_000_000))
    };
    let bytes_sent = if rng.gen_bool(prob_measurement_present) {
        None
    } else {
        Some(rng.gen_range(100_000..1_000_000_000))
    };
    ReputationMeasurements {
        latency,
        interactions,
        inbound_bandwidth,
        outbound_bandwidth,
        bytes_received,
        bytes_sent,
        hops: None,
    }
}

async fn run_transaction(
    requests: Vec<UpdateRequest>,
    update_socket: &Socket<Block, BlockExecutionResponse>,
) -> Result<BlockExecutionResponse> {
    let res = update_socket
        .run(Block {
            transactions: requests,
        })
        .await
        .map_err(|r| anyhow!(format!("{r:?}")))?;
    Ok(res)
}

async fn deposit(
    amount: BigDecimal<18>,
    token: Tokens,
    sender: AccountOwnerPublicKey,
    update_socket: &Socket<Block, BlockExecutionResponse>,
) {
    // Deposit some FLK into account 1
    let req = get_update_request_account(
        UpdateMethod::Deposit {
            proof: ProofOfConsensus {},
            token,
            amount,
        },
        sender,
    );
    run_transaction(vec![req], update_socket).await.unwrap();
}

async fn stake_lock(
    locked_for: u64,
    node: NodePublicKey,
    sender: AccountOwnerPublicKey,
    update_socket: &Socket<Block, BlockExecutionResponse>,
) {
    // Deposit some FLK into account 1
    let req = get_update_request_account(UpdateMethod::StakeLock { node, locked_for }, sender);
    run_transaction(vec![req], update_socket).await.unwrap();
}

async fn stake(
    amount: BigDecimal<18>,
    node_public_key: NodePublicKey,
    sender: AccountOwnerPublicKey,
    update_socket: &Socket<Block, BlockExecutionResponse>,
) {
    let update = get_update_request_account(
        UpdateMethod::Stake {
            amount,
            node_public_key,
            node_network_key: Some([0; 32].into()),
            node_domain: Some("/ip4/127.0.0.1/udp/38000".to_string()),
            worker_public_key: Some([0; 32].into()),
            worker_domain: Some("/ip4/127.0.0.1/udp/38000".to_string()),
            worker_mempool_address: Some("/ip4/127.0.0.1/udp/38000".to_string()),
        },
        sender,
    );
    if let TransactionResponse::Revert(error) = run_transaction(vec![update], update_socket)
        .await
        .unwrap()
        .txn_receipts[0]
        .clone()
    {
        panic!("Stake reverted: {error:?}");
    }
}

#[test]
async fn test_genesis() {
    // Init application + get the query and update socket
    let (_, query_runner) = init_app(None).await;
    // Get the genesis paramaters plus the initial committee
    let (genesis, genesis_committee) = get_genesis();
    // For every member of the genesis committee they should have an initial stake of the min stake
    // Query to make sure that holds true
    for node in genesis_committee {
        let balance = query_runner.get_staked(&node.public_key);
        assert_eq!(
            BigDecimal::<18>::from(genesis.min_stake.parse::<u128>().unwrap()),
            balance
        );
    }
}

#[test]
async fn test_epoch_change() {
    // Init application + get the query and update socket
    let (update_socket, query_runner) = init_app(None).await;
    let (_, genesis_committee) = get_genesis();

    let required_signals = 2 * genesis_committee.len() / 3 + 1;

    // Have (required_signals - 1) say they are ready to change epoch
    // make sure the epoch doesnt change each time someone signals
    for node in genesis_committee.iter().take(required_signals - 1) {
        let req = get_update_request_node(UpdateMethod::ChangeEpoch { epoch: 0 }, node.public_key);

        let res = run_transaction(vec![req], &update_socket).await.unwrap();
        // Make sure epoch didnt change
        assert!(!res.change_epoch);
    }
    // check that the current epoch is still 0
    assert_eq!(query_runner.get_epoch_info().epoch, 0);

    // Have the last needed committee member signal the epoch change and make sure it changes
    let req = get_update_request_node(
        UpdateMethod::ChangeEpoch { epoch: 0 },
        genesis_committee[required_signals].public_key,
    );
    let res = run_transaction(vec![req], &update_socket).await.unwrap();
    assert!(res.change_epoch);

    // Query epoch info and make sure it incremented to new epoch
    assert_eq!(query_runner.get_epoch_info().epoch, 1);
}

#[test]
async fn test_stake() {
    let (update_socket, query_runner) = init_app(None).await;
    let (genesis, _) = get_genesis();
    let node_public_key: NodePublicKey = BLS12381PublicKey::decode_base64(NODE_ONE)
        .unwrap()
        .pubkey
        .to_bytes()
        .into();

    // Deposit some FLK into account 1
    let update = get_update_request_account(
        UpdateMethod::Deposit {
            proof: ProofOfConsensus {},
            token: Tokens::FLK,
            amount: 1_000_u64.into(),
        },
        ACCOUNT_ONE,
    );
    // Put 2 of the transaction in the block just to also test block exucution a bit
    run_transaction(vec![update.clone(), update], &update_socket)
        .await
        .unwrap();

    // check that he has 2_000 flk balance
    assert_eq!(query_runner.get_flk_balance(&ACCOUNT_ONE), 2_000_u64.into());

    // Test staking on a new node

    // First check that trying to stake without providing all the node info reverts
    let update = get_update_request_account(
        UpdateMethod::Stake {
            amount: 1_000_u64.into(),
            node_public_key,
            node_network_key: None,
            node_domain: None,
            worker_public_key: None,
            worker_domain: None,
            worker_mempool_address: None,
        },
        ACCOUNT_ONE,
    );
    let res = run_transaction(vec![update], &update_socket).await.unwrap();

    assert_eq!(
        TransactionResponse::Revert(ExecutionError::InsufficientNodeDetails),
        res.txn_receipts[0]
    );

    // Now try with the correct details for a new node
    let update = get_update_request_account(
        UpdateMethod::Stake {
            amount: 1_000_u64.into(),
            node_public_key,
            node_network_key: Some([0; 32].into()),
            node_domain: Some("/ip4/127.0.0.1/udp/38000".to_string()),
            worker_public_key: Some([0; 32].into()),
            worker_domain: Some("/ip4/127.0.0.1/udp/38000".to_string()),
            worker_mempool_address: Some("/ip4/127.0.0.1/udp/38000".to_string()),
        },
        ACCOUNT_ONE,
    );
    if let TransactionResponse::Revert(error) = run_transaction(vec![update], &update_socket)
        .await
        .unwrap()
        .txn_receipts[0]
        .clone()
    {
        panic!("Stake reverted: {error:?}");
    }

    // Query the new node and make sure he has the proper stake
    assert_eq!(query_runner.get_staked(&node_public_key), 1_000_u64.into());

    // Stake 1000 more but since it is not a new node we should be able to leave the optional
    // paramaters out without a revert
    let update = get_update_request_account(
        UpdateMethod::Stake {
            amount: 1_000_u64.into(),
            node_public_key,
            node_network_key: None,
            node_domain: None,
            worker_public_key: None,
            worker_domain: None,
            worker_mempool_address: None,
        },
        ACCOUNT_ONE,
    );
    if let TransactionResponse::Revert(error) = run_transaction(vec![update], &update_socket)
        .await
        .unwrap()
        .txn_receipts[0]
        .clone()
    {
        panic!("Stake reverted: {error:?}");
    }

    // Node should now have 2_000 stake
    assert_eq!(query_runner.get_staked(&node_public_key), 2_000_u64.into());

    // Now test unstake and make sure it moves the tokens to locked status
    let update = get_update_request_account(
        UpdateMethod::Unstake {
            amount: 1_000_u64.into(),
            node: node_public_key,
        },
        ACCOUNT_ONE,
    );
    run_transaction(vec![update], &update_socket).await.unwrap();

    // Check that his locked is 1000 and his remaining stake is 1000
    assert_eq!(query_runner.get_staked(&node_public_key), 1_000_u64.into());
    assert_eq!(query_runner.get_locked(&node_public_key), 1_000_u64.into());
    // Since this test starts at epoch 0 locked_until will be == lock_time
    assert_eq!(
        query_runner.get_locked_time(&node_public_key),
        genesis.lock_time.parse::<u64>().unwrap()
    );

    // Try to withdraw the locked tokens and it should revery
    let update = get_update_request_account(
        UpdateMethod::WithdrawUnstaked {
            node: node_public_key,
            recipient: None,
        },
        ACCOUNT_ONE,
    );
    let res = run_transaction(vec![update], &update_socket)
        .await
        .unwrap()
        .txn_receipts[0]
        .clone();
    assert_eq!(
        TransactionResponse::Revert(ExecutionError::TokensLocked),
        res
    );
}

#[test]
async fn test_stake_lock() {
    let (update_socket, query_runner) = init_app(None).await;
    let node_public_key: NodePublicKey = BLS12381PublicKey::decode_base64(NODE_ONE)
        .unwrap()
        .pubkey
        .to_bytes()
        .into();

    deposit(1_000_u64.into(), Tokens::FLK, ACCOUNT_ONE, &update_socket).await;
    assert_eq!(query_runner.get_flk_balance(&ACCOUNT_ONE), 1_000_u64.into());

    stake(
        1_000_u64.into(),
        node_public_key,
        ACCOUNT_ONE,
        &update_socket,
    )
    .await;
    assert_eq!(query_runner.get_staked(&node_public_key), 1_000_u64.into());

    let stake_lock_req = get_update_request_account(
        UpdateMethod::StakeLock {
            node: node_public_key,
            locked_for: 365,
        },
        ACCOUNT_ONE,
    );

    if let TransactionResponse::Revert(error) =
        run_transaction(vec![stake_lock_req], &update_socket)
            .await
            .unwrap()
            .txn_receipts[0]
            .clone()
    {
        panic!("Stake locking reverted: {error:?}");
    }
    assert_eq!(query_runner.get_stake_locked_until(&node_public_key), 365);

    let unstake_req = get_update_request_account(
        UpdateMethod::Unstake {
            amount: 1_000_u64.into(),
            node: node_public_key,
        },
        ACCOUNT_ONE,
    );
    let res = run_transaction(vec![unstake_req], &update_socket)
        .await
        .unwrap()
        .txn_receipts[0]
        .clone();

    assert_eq!(
        res,
        TransactionResponse::Revert(ExecutionError::LockedTokensUnstakeForbidden)
    );
}

#[test]
async fn test_pod_without_proof() {
    let (update_socket, query_runner) = init_app(None).await;

    // use a node from a genesis committee for testing
    let node_key = "l0Jel6KEFG7H6sV2nWKOQxDaMKWMeiUBqK5VHKcStWrLPHAANRB+dt7gp0jQ7ooxEaI7ukOQZk6U5vcL7ESHA1J/iAWQ7YNO/ZCvR1pfWfcTNBONIzeiUWAN+iyKfV10";
    let node_public_key = BLS12381PublicKey::decode_base64(node_key)
        .unwrap()
        .pubkey
        .to_bytes()
        .into();

    let bandwidth_pod = pod_request(node_public_key, 1000, 0);
    let compute_pod = pod_request(node_public_key, 2000, 1);

    // run the delivery ack transaction
    if let Err(e) = run_transaction(vec![bandwidth_pod, compute_pod], &update_socket).await {
        panic!("{e}");
    }

    assert_eq!(
        query_runner.get_commodity_served(&node_public_key),
        vec![1000, 2000]
    );

    assert_eq!(
        query_runner.get_total_served(0),
        TotalServed {
            served: vec![1000, 2000],
            reward_pool: (0.1 * 1000_f64 + 0.2 * 2000_f64)
        }
    );
}

#[test]
async fn test_distribute_rewards() {
    let max_inflation = "10".to_owned();
    let protocol_part = "15".to_owned();
    let node_part = "80".to_owned();
    let validator_part = "5".to_owned();
    let boost = "4".to_owned();
    let supply_at_genesis = "1000000".to_owned();
    let (update_socket, query_runner) = init_app_with_params(Params {
        epoch_time: None,
        max_inflation: Some(max_inflation.clone()),
        protocol_share: Some(protocol_part.clone()),
        node_share: Some(node_part.clone()),
        validator_share: Some(validator_part.clone()),
        max_boost: Some(boost.clone()),
        supply_at_genesis: Some(supply_at_genesis.clone()),
    })
    .await;

    let (owner_key1, node_key1) = node_and_account_key(3);
    let (owner_key2, node_key2) = node_and_account_key(4);

    // get params for emission calculations
    let supply_at_year_start: BigDecimal<18> = supply_at_genesis.parse().unwrap_or(0.0).into();
    let inflation: BigDecimal<18> = max_inflation.parse().unwrap_or(0.0).into();
    let node_share = BigDecimal::from(node_part.parse().unwrap_or(0.0) / 100.0);
    let validator_share = BigDecimal::from(validator_part.parse().unwrap_or(0.0) / 100.0);
    let protocol_share = BigDecimal::from(protocol_part.parse().unwrap_or(0.0) / 100.0);
    let max_boost: BigDecimal<18> = boost.parse().unwrap_or(0.0).into();

    // deposit FLK tokens and stake it
    deposit(10_000_u64.into(), Tokens::FLK, owner_key1, &update_socket).await;
    stake(10_000_u64.into(), node_key1, owner_key1, &update_socket).await;
    deposit(10_000_u64.into(), Tokens::FLK, owner_key2, &update_socket).await;
    stake(10_000_u64.into(), node_key2, owner_key2, &update_socket).await;
    // staking locking for four year to get max boosts
    stake_lock(1460, node_key1, owner_key1, &update_socket).await;
    let node_1_boost = &max_boost;

    // submit pods for usage
    let pod_10 = pod_request(node_key1, 10000, 0);
    let pod11 = pod_request(node_key1, 6767, 1);
    let pod_21 = pod_request(node_key2, 5000, 1);
    // run the delivery ack transaction
    if let Err(e) = run_transaction(vec![pod_10, pod11, pod_21], &update_socket).await {
        panic!("{e}");
    }

    // call epoch change that will trigger distribute rewards
    let committee_members = query_runner.get_committee_members();
    if let Err(err) = simple_epoch_change(0, &committee_members, &update_socket).await {
        panic!("error while changing epoch, {err}");
    }
    let node_1_usd = 0.1 * 10000_f64 + 0.2 * 6767_f64;
    let node_2_usd = 0.2 * 5000_f64;
    let reward_pool: BigDecimal<18> = (node_2_usd + node_1_usd).into();

    // assert stable balances
    let stables_balance = query_runner.get_stables_balance(&owner_key1);
    assert_eq!(stables_balance, node_1_usd.into());

    // calculate emissions per unit
    let max_emissions: BigDecimal<18> = (inflation * supply_at_year_start) / &36500.0.into();
    let emissions_per_unit = &max_emissions / (&reward_pool * &max_boost);

    // assert flk balances node 1
    let node_flk_balance1 = query_runner.get_flk_balance(&owner_key1);
    let node_flk_rewards1: BigDecimal<18> =
        &emissions_per_unit * &node_share * node_1_boost * &node_1_usd.into();
    assert_eq!(node_flk_balance1, node_flk_rewards1);

    // assert flk balances node 2
    let node_flk_balance2 = query_runner.get_flk_balance(&owner_key2);
    let node_flk_rewards2: BigDecimal<18> = &emissions_per_unit * &node_share * &node_2_usd.into();
    assert_eq!(node_flk_balance2, node_flk_rewards2);

    // calculate total emissions based on total emissions for node which is equal to node share. the
    // rest goes to other validators(maybe) and protocol
    let total_emissions: BigDecimal<18> = (&node_flk_rewards1 + &node_flk_rewards2) / &node_share;

    // assert protocols share
    let protocol_address = query_runner.get_protocol_params(ProtocolParams::ProtocolFundAddress);
    let protocol_account: AccountOwnerPublicKey = AccountOwnerPublicKey(
        Ed25519PublicKey::decode_base64(&protocol_address)
            .unwrap()
            .0
            .to_bytes(),
    );
    let protocol_balance = query_runner.get_flk_balance(&protocol_account);
    let protocol_rewards = &total_emissions * &protocol_share;
    assert_eq!(protocol_balance, protocol_rewards);

    // assert validaots share
    let committee_account1 = query_runner
        .get_node_info(&committee_members[0])
        .unwrap()
        .owner;

    let validator_balance = query_runner.get_flk_balance(&committee_account1);
    let validator_rewards = (&total_emissions * &validator_share) / &committee_members.len().into();
    assert_eq!(validator_balance, validator_rewards);
}

#[test]
async fn test_submit_rep_measurements() {
    // Init application + get the query and update socket
    let (update_socket, query_runner) = init_app(None).await;

    let mut map = BTreeMap::new();
    let mut rng = get_seedable_rng();

    let node_public_key: NodePublicKey = BLS12381PublicKey::decode_base64(NODE_ONE)
        .unwrap()
        .pubkey
        .to_bytes()
        .into();

    deposit(1_000_u64.into(), Tokens::FLK, ACCOUNT_ONE, &update_socket).await;
    stake(
        1_000_u64.into(),
        node_public_key,
        ACCOUNT_ONE,
        &update_socket,
    )
    .await;

    let measurements1 = generate_reputation_measurements(&mut rng, 0.1);
    let peer1 = NodePublicKey([0; 96]);
    map.insert(peer1, measurements1.clone());

    let measurements2 = generate_reputation_measurements(&mut rng, 0.1);
    let peer2 = NodePublicKey([1; 96]);
    map.insert(peer2, measurements2.clone());

    let req = get_update_request_node(
        UpdateMethod::SubmitReputationMeasurements { measurements: map },
        node_public_key,
    );
    if let Err(e) = run_transaction(vec![req], &update_socket).await {
        panic!("{e}");
    }

    let rep_measurements1 = query_runner.get_rep_measurements(peer1);
    assert_eq!(rep_measurements1.len(), 1);
    assert_eq!(rep_measurements1[0].reporting_node, node_public_key);
    assert_eq!(rep_measurements1[0].measurements, measurements1);

    let rep_measurements2 = query_runner.get_rep_measurements(peer2);
    assert_eq!(rep_measurements2.len(), 1);
    assert_eq!(rep_measurements2[0].reporting_node, node_public_key);
    assert_eq!(rep_measurements2[0].measurements, measurements2);
}

#[test]
async fn test_rep_scores() {
    // Init application + get the query and update socket
    let (update_socket, query_runner) = init_app(None).await;
    let (_, genesis_committee) = get_genesis();
    let required_signals = 2 * genesis_committee.len() / 3 + 1;

    let mut rng = get_seedable_rng();

    let mut array = [0; 96];
    (0..96).for_each(|i| array[i] = rng.gen_range(0..=255));
    let node_public_key1 = NodePublicKey(array);

    let mut array = [0; 96];
    (0..96).for_each(|i| array[i] = rng.gen_range(0..=255));
    let node_public_key2 = NodePublicKey(array);

    deposit(2_000_u64.into(), Tokens::FLK, ACCOUNT_ONE, &update_socket).await;
    stake(
        1_000_u64.into(),
        node_public_key1,
        ACCOUNT_ONE,
        &update_socket,
    )
    .await;
    stake(
        1_000_u64.into(),
        node_public_key2,
        ACCOUNT_ONE,
        &update_socket,
    )
    .await;

    let mut map = BTreeMap::new();
    let measurements = generate_reputation_measurements(&mut rng, 0.1);
    let peer1 = NodePublicKey([0; 96]);
    map.insert(peer1, measurements.clone());

    let measurements = generate_reputation_measurements(&mut rng, 0.1);
    let peer2 = NodePublicKey([1; 96]);
    map.insert(peer2, measurements.clone());

    let req = get_update_request_node(
        UpdateMethod::SubmitReputationMeasurements { measurements: map },
        node_public_key1,
    );

    if let Err(e) = run_transaction(vec![req], &update_socket).await {
        panic!("{e}");
    }

    let mut map = BTreeMap::new();
    let measurements = generate_reputation_measurements(&mut rng, 0.1);
    let peer1 = NodePublicKey([0; 96]);
    map.insert(peer1, measurements.clone());

    let measurements = generate_reputation_measurements(&mut rng, 0.1);
    let peer2 = NodePublicKey([1; 96]);
    map.insert(peer2, measurements.clone());

    let req = get_update_request_node(
        UpdateMethod::SubmitReputationMeasurements { measurements: map },
        node_public_key2,
    );

    if let Err(e) = run_transaction(vec![req], &update_socket).await {
        panic!("{e}");
    }

    // Change epoch so that rep scores will be calculated from the measurements.
    for node in genesis_committee.iter().take(required_signals) {
        let req = get_update_request_node(UpdateMethod::ChangeEpoch { epoch: 0 }, node.public_key);
        run_transaction(vec![req], &update_socket).await.unwrap();
    }

    assert!(query_runner.get_reputation(&peer1).is_some());
    assert!(query_runner.get_reputation(&peer2).is_some());
}

#[test]
#[ignore = "some minute precision errors"]
async fn test_supply_across_epoch() {
    let epoch_time = "100".to_string();
    let max_inflation = "10".to_owned();
    let protocol_part = "10".to_owned();
    let node_part = "85".to_owned();
    let validator_part = "5".to_owned();
    let boost = "4".to_owned();
    let supply_at_genesis = "1000000".to_owned();
    let (update_socket, query_runner) = init_app_with_params(Params {
        epoch_time: Some(epoch_time.clone()),
        max_inflation: Some(max_inflation.clone()),
        protocol_share: Some(protocol_part.clone()),
        node_share: Some(node_part.clone()),
        validator_share: Some(validator_part.clone()),
        max_boost: Some(boost.clone()),
        supply_at_genesis: Some(supply_at_genesis.clone()),
    })
    .await;

    // get params for emission calculations
    let supply_at_year_start: BigDecimal<18> = supply_at_genesis.parse().unwrap_or(0.0).into();
    let inflation: BigDecimal<18> = max_inflation.parse().unwrap_or(0.0).into();
    let node_share = node_part.parse().unwrap_or(0.0) / 100.0;
    let validator_share = validator_part.parse().unwrap_or(0.0) / 400.0;
    let protocol_share = protocol_part.parse().unwrap_or(0.0) / 100.0;
    let max_boost: BigDecimal<18> = boost.parse().unwrap_or(0.0).into();

    let (owner_key1, node_key1) = node_and_account_key(5);

    // deposit FLK tokens and stake it
    deposit(10_000_u64.into(), Tokens::FLK, owner_key1, &update_socket).await;
    stake(10_000_u64.into(), node_key1, owner_key1, &update_socket).await;

    //every epoch supply increase similar for simplicity of the test
    let _node_1_usd = 0.1 * 10000_f64;

    // calculate emissions per unit
    let max_emissions: BigDecimal<18> = (&inflation * &supply_at_year_start) / &36500.0.into();

    let emissions_per_epoch = &max_emissions / &max_boost;
    let mut supply = supply_at_year_start;

    // 365 epoch changes to see if the current supply and year start suppply are ok
    for i in 0..2 {
        let pod_10 = pod_request(node_key1, 10000, 0);
        // run the delivery ack transaction
        if let Err(e) = run_transaction(vec![pod_10], &update_socket).await {
            panic!("{e}");
        }
        let committee_members = query_runner.get_committee_members();
        if let Err(err) = simple_epoch_change(i, &committee_members, &update_socket).await {
            panic!("error while changing epoch, {err}");
        }
        println!(
            "{}\n{}\n{}\n{}\n{}\n{}\n",
            (&emissions_per_epoch * &node_share.into()),
            (&emissions_per_epoch * &validator_share.into()),
            (&emissions_per_epoch * &validator_share.into()),
            (&emissions_per_epoch * &validator_share.into()),
            (&emissions_per_epoch * &validator_share.into()),
            (&emissions_per_epoch * &protocol_share.into())
        );
        println!(
            "{}\n",
            (&emissions_per_epoch * &node_share.into())
                + (&emissions_per_epoch * &validator_share.into())
                + (&emissions_per_epoch * &validator_share.into())
                + (&emissions_per_epoch * &validator_share.into())
                + (&emissions_per_epoch * &validator_share.into())
                + (&emissions_per_epoch * &protocol_share.into())
                + (&1_000_000_u128.into())
        );
        let epoch = query_runner.get_epoch_info().epoch;
        let total_supply = query_runner.get_total_supply();
        let supply_year_start = query_runner.get_year_start_supply();
        supply += emissions_per_epoch.clone();
        println!("{total_supply} \n{supply}");
        assert_eq!(total_supply, supply);
        println!("epoch: {epoch}, suppply: {total_supply}, supply_year_start: {supply_year_start}")
    }
}
