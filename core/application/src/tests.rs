use std::{collections::BTreeMap, time::SystemTime, vec};

use affair::Socket;
use anyhow::{anyhow, Result};
use fleek_crypto::{
    AccountOwnerSecretKey, NodeNetworkingSecretKey, NodePublicKey, NodeSecretKey, PublicKey,
    SecretKey,
};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::{
    application::ExecutionEngineSocket,
    types::{
        Block, Epoch, ExecutionError, NodeInfo, ProofOfConsensus, Tokens, TotalServed,
        TransactionResponse, UpdateMethod, UpdatePayload, UpdateRequest,
    },
    ApplicationInterface, BlockExecutionResponse, DeliveryAcknowledgment, SyncQueryRunnerInterface,
    ToDigest,
};
use lightning_test_utils::{random, reputation};
use tokio::test;

use crate::{
    app::Application,
    config::{Config, Mode},
    genesis::{Genesis, GenesisCommittee},
    query_runner::QueryRunner,
};

pub struct Params {
    epoch_time: Option<u64>,
    max_inflation: Option<u16>,
    protocol_share: Option<u16>,
    node_share: Option<u16>,
    service_builder_share: Option<u16>,
    max_boost: Option<u16>,
    supply_at_genesis: Option<u64>,
}

struct GenesisCommitteeKeystore {
    _owner_secret_key: AccountOwnerSecretKey,
    node_secret_key: NodeSecretKey,
    _network_secret_key: NodeNetworkingSecretKey,
    _worker_secret_key: NodeNetworkingSecretKey,
}

fn get_genesis_committee(
    num_members: usize,
) -> (Vec<GenesisCommittee>, Vec<GenesisCommitteeKeystore>) {
    let mut keystore = Vec::new();
    let mut committee = Vec::new();
    (0..num_members).for_each(|i| {
        let node_secret_key = NodeSecretKey::generate();
        let node_public_key = node_secret_key.to_pk();
        let network_secret_key = NodeNetworkingSecretKey::generate();
        let network_public_key = network_secret_key.to_pk();
        let owner_secret_key = AccountOwnerSecretKey::generate();
        let owner_public_key = owner_secret_key.to_pk();
        committee.push(GenesisCommittee::new(
            owner_public_key.to_base64(),
            node_public_key.to_base64(),
            format!("/ip4/127.0.0.1/udp/800{i}"),
            network_public_key.to_base64(),
            format!("/ip4/127.0.0.1/udp/810{i}/http"),
            network_public_key.to_base64(),
            format!("/ip4/127.0.0.1/tcp/810{i}/http"),
            None,
        ));
        keystore.push(GenesisCommitteeKeystore {
            _owner_secret_key: owner_secret_key,
            node_secret_key,
            _network_secret_key: network_secret_key,
            _worker_secret_key: network_secret_key,
        });
    });
    (committee, keystore)
}

// Init the app and return the execution engine socket that would go to narwhal and the query socket
// that could go to anyone
async fn init_app(config: Option<Config>) -> (ExecutionEngineSocket, QueryRunner) {
    let config = config.or_else(|| Some(Config::default()));
    let app = Application::init(config.unwrap()).await.unwrap();

    (app.transaction_executor(), app.sync_query())
}

async fn init_app_with_params(
    params: Params,
    committee: Option<Vec<GenesisCommittee>>,
) -> (ExecutionEngineSocket, QueryRunner) {
    let mut genesis = Genesis::load().expect("Failed to load genesis from file.");

    if let Some(committee) = committee {
        genesis.committee = committee;
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
    };

    init_app(Some(config)).await
}

async fn simple_epoch_change(
    epoch: Epoch,
    committee_keystore: &Vec<GenesisCommitteeKeystore>,
    update_socket: &Socket<Block, BlockExecutionResponse>,
    nonce: u64,
) -> Result<()> {
    let required_signals = 2 * committee_keystore.len() / 3 + 1;
    // make call epoch change for 2/3rd committe members
    for (index, node) in committee_keystore.iter().enumerate().take(required_signals) {
        let req = get_update_request_node(
            UpdateMethod::ChangeEpoch { epoch },
            node.node_secret_key,
            nonce,
        );
        let res = run_transaction(vec![req], update_socket).await?;
        // check epoch change

        if index == required_signals - 1 {
            assert!(res.change_epoch);
        }
    }
    Ok(())
}

// Helper method to get a transaction update request from a node.
// Passing the private key around like this should only be done for
// testing.
fn get_update_request_node(
    method: UpdateMethod,
    secret_key: NodeSecretKey,
    nonce: u64,
) -> UpdateRequest {
    let payload = UpdatePayload { nonce, method };
    let digest = payload.to_digest();
    let signature = secret_key.sign(&digest);
    UpdateRequest {
        sender: secret_key.to_pk().into(),
        signature: signature.into(),
        payload,
    }
}
// Passing the private key around like this should only be done for
// testing.
fn get_update_request_account(
    method: UpdateMethod,
    secret_key: AccountOwnerSecretKey,
    nonce: u64,
) -> UpdateRequest {
    let payload = UpdatePayload { nonce, method };
    let digest = payload.to_digest();
    let signature = secret_key.sign(&digest);
    UpdateRequest {
        sender: secret_key.to_pk().into(),
        signature: signature.into(),
        payload,
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
// Passing the private key around like this should only be done for
// testing.
fn pod_request(
    secret_key: NodeSecretKey,
    commodity: u128,
    service_id: u32,
    nonce: u64,
) -> UpdateRequest {
    get_update_request_node(
        UpdateMethod::SubmitDeliveryAcknowledgmentAggregation {
            commodity,  // units of data served
            service_id, // service 0 serving bandwidth
            proofs: vec![DeliveryAcknowledgment::default()],
            metadata: None,
        },
        secret_key,
        nonce,
    )
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
    amount: HpUfixed<18>,
    token: Tokens,
    secret_key: AccountOwnerSecretKey,
    update_socket: &Socket<Block, BlockExecutionResponse>,
    nonce: u64,
) {
    // Deposit some FLK into account 1
    let req = get_update_request_account(
        UpdateMethod::Deposit {
            proof: ProofOfConsensus {},
            token,
            amount,
        },
        secret_key,
        nonce,
    );
    run_transaction(vec![req], update_socket).await.unwrap();
}

async fn stake_lock(
    locked_for: u64,
    node: NodePublicKey,
    secret_key: AccountOwnerSecretKey,
    update_socket: &Socket<Block, BlockExecutionResponse>,
    nonce: u64,
) {
    // Deposit some FLK into account 1
    let req = get_update_request_account(
        UpdateMethod::StakeLock { node, locked_for },
        secret_key,
        nonce,
    );
    run_transaction(vec![req], update_socket).await.unwrap();
}

async fn stake(
    amount: HpUfixed<18>,
    node_public_key: NodePublicKey,
    secret_key: AccountOwnerSecretKey,
    update_socket: &Socket<Block, BlockExecutionResponse>,
    nonce: u64,
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
        secret_key,
        nonce,
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
        assert_eq!(HpUfixed::<18>::from(genesis.min_stake), balance);
    }
}

#[test]
async fn test_epoch_change() {
    let (committee, keystore) = get_genesis_committee(4);
    let mut genesis = Genesis::load().unwrap();
    let committee_size = committee.len();
    genesis.committee = committee;
    let (update_socket, query_runner) = init_app(Some(Config {
        genesis: Some(genesis),
        mode: Mode::Test,
    }))
    .await;

    let required_signals = 2 * committee_size / 3 + 1;

    // Have (required_signals - 1) say they are ready to change epoch
    // make sure the epoch doesnt change each time someone signals
    for node in keystore.iter().take(required_signals - 1) {
        let req = get_update_request_node(
            UpdateMethod::ChangeEpoch { epoch: 0 },
            node.node_secret_key,
            1,
        );

        let res = run_transaction(vec![req], &update_socket).await.unwrap();
        // Make sure epoch didnt change
        assert!(!res.change_epoch);
    }
    // check that the current epoch is still 0
    assert_eq!(query_runner.get_epoch_info().epoch, 0);

    // Have the last needed committee member signal the epoch change and make sure it changes
    let req = get_update_request_node(
        UpdateMethod::ChangeEpoch { epoch: 0 },
        keystore[required_signals].node_secret_key,
        1,
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

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_secret_key = NodeSecretKey::generate();

    // Deposit some FLK into account 1
    let update1 = get_update_request_account(
        UpdateMethod::Deposit {
            proof: ProofOfConsensus {},
            token: Tokens::FLK,
            amount: 1_000_u64.into(),
        },
        owner_secret_key,
        1,
    );
    let update2 = get_update_request_account(
        UpdateMethod::Deposit {
            proof: ProofOfConsensus {},
            token: Tokens::FLK,
            amount: 1_000_u64.into(),
        },
        owner_secret_key,
        2,
    );
    // Put 2 of the transaction in the block just to also test block exucution a bit
    run_transaction(vec![update1, update2], &update_socket)
        .await
        .unwrap();

    // check that he has 2_000 flk balance
    assert_eq!(
        query_runner.get_flk_balance(&owner_secret_key.to_pk().into()),
        2_000_u64.into()
    );

    // Test staking on a new node

    // First check that trying to stake without providing all the node info reverts
    let update = get_update_request_account(
        UpdateMethod::Stake {
            amount: 1_000_u64.into(),
            node_public_key: node_secret_key.to_pk(),
            node_network_key: None,
            node_domain: None,
            worker_public_key: None,
            worker_domain: None,
            worker_mempool_address: None,
        },
        owner_secret_key,
        3,
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
            node_public_key: node_secret_key.to_pk(),
            node_network_key: Some([0; 32].into()),
            node_domain: Some("/ip4/127.0.0.1/udp/38000".to_string()),
            worker_public_key: Some([0; 32].into()),
            worker_domain: Some("/ip4/127.0.0.1/udp/38000".to_string()),
            worker_mempool_address: Some("/ip4/127.0.0.1/udp/38000".to_string()),
        },
        owner_secret_key,
        4,
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
    assert_eq!(
        query_runner.get_staked(&node_secret_key.to_pk()),
        1_000_u64.into()
    );

    // Stake 1000 more but since it is not a new node we should be able to leave the optional
    // paramaters out without a revert
    let update = get_update_request_account(
        UpdateMethod::Stake {
            amount: 1_000_u64.into(),
            node_public_key: node_secret_key.to_pk(),
            node_network_key: None,
            node_domain: None,
            worker_public_key: None,
            worker_domain: None,
            worker_mempool_address: None,
        },
        owner_secret_key,
        5,
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
    assert_eq!(
        query_runner.get_staked(&node_secret_key.to_pk()),
        2_000_u64.into()
    );

    // Now test unstake and make sure it moves the tokens to locked status
    let update = get_update_request_account(
        UpdateMethod::Unstake {
            amount: 1_000_u64.into(),
            node: node_secret_key.to_pk(),
        },
        owner_secret_key,
        6,
    );
    run_transaction(vec![update], &update_socket).await.unwrap();

    // Check that his locked is 1000 and his remaining stake is 1000
    assert_eq!(
        query_runner.get_staked(&node_secret_key.to_pk()),
        1_000_u64.into()
    );
    assert_eq!(
        query_runner.get_locked(&node_secret_key.to_pk()),
        1_000_u64.into()
    );
    // Since this test starts at epoch 0 locked_until will be == lock_time
    assert_eq!(
        query_runner.get_locked_time(&node_secret_key.to_pk()),
        genesis.lock_time
    );

    // Try to withdraw the locked tokens and it should revery
    let update = get_update_request_account(
        UpdateMethod::WithdrawUnstaked {
            node: node_secret_key.to_pk(),
            recipient: None,
        },
        owner_secret_key,
        7,
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

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_secret_key = NodeSecretKey::generate();

    deposit(
        1_000_u64.into(),
        Tokens::FLK,
        owner_secret_key,
        &update_socket,
        1,
    )
    .await;
    assert_eq!(
        query_runner.get_flk_balance(&owner_secret_key.to_pk().into()),
        1_000_u64.into()
    );

    stake(
        1_000_u64.into(),
        node_secret_key.to_pk(),
        owner_secret_key,
        &update_socket,
        2,
    )
    .await;
    assert_eq!(
        query_runner.get_staked(&node_secret_key.to_pk()),
        1_000_u64.into()
    );

    let stake_lock_req = get_update_request_account(
        UpdateMethod::StakeLock {
            node: node_secret_key.to_pk(),
            locked_for: 365,
        },
        owner_secret_key,
        3,
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
    assert_eq!(
        query_runner.get_stake_locked_until(&node_secret_key.to_pk()),
        365
    );

    let unstake_req = get_update_request_account(
        UpdateMethod::Unstake {
            amount: 1_000_u64.into(),
            node: node_secret_key.to_pk(),
        },
        owner_secret_key,
        4,
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
    let (committee, keystore) = get_genesis_committee(4);
    let mut genesis = Genesis::load().unwrap();
    genesis.committee = committee;
    let (update_socket, query_runner) = init_app(Some(Config {
        genesis: Some(genesis),
        mode: Mode::Test,
    }))
    .await;

    let bandwidth_pod = pod_request(keystore[0].node_secret_key, 1000, 0, 1);
    let compute_pod = pod_request(keystore[0].node_secret_key, 2000, 1, 2);

    // run the delivery ack transaction
    if let Err(e) = run_transaction(vec![bandwidth_pod, compute_pod], &update_socket).await {
        panic!("{e}");
    }

    assert_eq!(
        query_runner
            .get_node_served(&keystore[0].node_secret_key.to_pk())
            .served,
        vec![1000, 2000]
    );

    assert_eq!(
        query_runner.get_total_served(0),
        TotalServed {
            served: vec![1000, 2000],
            reward_pool: (0.1 * 1000_f64 + 0.2 * 2000_f64).into()
        }
    );
}

#[test]
async fn test_distribute_rewards() {
    let (committee, keystore) = get_genesis_committee(4);

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
    )
    .await;

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

    // deposit FLK tokens and stake it
    deposit(
        10_000_u64.into(),
        Tokens::FLK,
        owner_secret_key1,
        &update_socket,
        1,
    )
    .await;
    stake(
        10_000_u64.into(),
        node_secret_key1.to_pk(),
        owner_secret_key1,
        &update_socket,
        2,
    )
    .await;
    deposit(
        10_000_u64.into(),
        Tokens::FLK,
        owner_secret_key2,
        &update_socket,
        1,
    )
    .await;
    stake(
        10_000_u64.into(),
        node_secret_key2.to_pk(),
        owner_secret_key2,
        &update_socket,
        2,
    )
    .await;
    // staking locking for 4 year to get boosts
    stake_lock(
        1460,
        node_secret_key2.to_pk(),
        owner_secret_key2,
        &update_socket,
        3,
    )
    .await;

    // submit pods for usage
    let pod_10 = pod_request(node_secret_key1, 12_800, 0, 1);
    let pod11 = pod_request(node_secret_key1, 3_600, 1, 2);
    let pod_21 = pod_request(node_secret_key2, 5000, 1, 1);

    let node_1_usd = 0.1 * 12_800_f64 + 0.2 * 3_600_f64; // 2_000 in revenue
    let node_2_usd = 0.2 * 5_000_f64; // 1_000 in revenue
    let reward_pool: HpUfixed<6> = (node_1_usd + node_2_usd).into();

    let node_1_proportion: HpUfixed<18> = HpUfixed::from(2000_u64) / HpUfixed::from(3000_u64);
    let node_2_proportion: HpUfixed<18> = HpUfixed::from(1000_u64) / HpUfixed::from(3000_u64);

    let service_proportions: Vec<HpUfixed<18>> = vec![
        HpUfixed::from(1280_u64) / HpUfixed::from(3000_u64),
        HpUfixed::from(1720_u64) / HpUfixed::from(3000_u64),
    ];
    // run the delivery ack transaction
    if let Err(e) = run_transaction(vec![pod_10, pod11, pod_21], &update_socket).await {
        panic!("{e}");
    }

    // call epoch change that will trigger distribute rewards
    if let Err(err) = simple_epoch_change(0, &keystore, &update_socket, 1).await {
        panic!("error while changing epoch, {err}");
    }

    // assert stable balances
    let stables_balance = query_runner.get_stables_balance(&owner_secret_key1.to_pk().into());
    assert_eq!(
        stables_balance,
        <f64 as Into<HpUfixed<6>>>::into(node_1_usd) * node_share.convert_precision()
    );
    let stables_balance2 = query_runner.get_stables_balance(&owner_secret_key2.to_pk().into());
    assert_eq!(
        stables_balance2,
        <f64 as Into<HpUfixed<6>>>::into(node_2_usd) * node_share.convert_precision()
    );

    let total_share =
        &node_1_proportion * HpUfixed::from(1_u64) + &node_2_proportion * HpUfixed::from(4_u64);

    // calculate emissions per unit
    let emissions: HpUfixed<18> = (inflation * supply_at_year_start) / &365.0.into();
    let emissions_for_node = &emissions * &node_share;

    // assert flk balances node 1
    let node_flk_balance1 = query_runner.get_flk_balance(&owner_secret_key1.to_pk().into());
    let node_flk_rewards1 = (&emissions_for_node * &node_1_proportion) / &total_share;
    assert_eq!(node_flk_balance1, node_flk_rewards1);

    // assert flk balances node 2
    let node_flk_balance2 = query_runner.get_flk_balance(&owner_secret_key2.to_pk().into());
    let node_flk_rewards2: HpUfixed<18> =
        (&emissions_for_node * (&node_2_proportion * HpUfixed::from(4_u64))) / &total_share;
    assert_eq!(node_flk_balance2, node_flk_rewards2);

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

#[test]
async fn test_submit_rep_measurements() {
    let (committee, keystore) = get_genesis_committee(4);
    let mut genesis = Genesis::load().unwrap();
    genesis.committee = committee;
    let (update_socket, query_runner) = init_app(Some(Config {
        genesis: Some(genesis),
        mode: Mode::Test,
    }))
    .await;

    let mut map = BTreeMap::new();
    let mut rng = random::get_seedable_rng();

    let measurements1 = reputation::generate_reputation_measurements(&mut rng, 0.1);
    let peer1 = NodePublicKey([0; 96]);
    map.insert(peer1, measurements1.clone());

    let measurements2 = reputation::generate_reputation_measurements(&mut rng, 0.1);
    let peer2 = NodePublicKey([1; 96]);
    map.insert(peer2, measurements2.clone());

    let req = get_update_request_node(
        UpdateMethod::SubmitReputationMeasurements { measurements: map },
        keystore[0].node_secret_key,
        1,
    );
    if let Err(e) = run_transaction(vec![req], &update_socket).await {
        panic!("{e}");
    }

    let rep_measurements1 = query_runner.get_rep_measurements(peer1);
    assert_eq!(rep_measurements1.len(), 1);
    assert_eq!(
        rep_measurements1[0].reporting_node,
        keystore[0].node_secret_key.to_pk()
    );
    assert_eq!(rep_measurements1[0].measurements, measurements1);

    let rep_measurements2 = query_runner.get_rep_measurements(peer2);
    assert_eq!(rep_measurements2.len(), 1);
    assert_eq!(
        rep_measurements2[0].reporting_node,
        keystore[0].node_secret_key.to_pk()
    );
    assert_eq!(rep_measurements2[0].measurements, measurements2);
}

#[test]
async fn test_rep_scores() {
    let (committee, keystore) = get_genesis_committee(4);
    let committee_len = committee.len();
    let mut genesis = Genesis::load().unwrap();
    genesis.committee = committee;
    let (update_socket, query_runner) = init_app(Some(Config {
        genesis: Some(genesis),
        mode: Mode::Test,
    }))
    .await;
    let required_signals = 2 * committee_len / 3 + 1;

    let mut rng = random::get_seedable_rng();

    let mut map = BTreeMap::new();
    let measurements = reputation::generate_reputation_measurements(&mut rng, 0.1);
    let peer1 = NodePublicKey([0; 96]);
    map.insert(peer1, measurements.clone());

    let measurements = reputation::generate_reputation_measurements(&mut rng, 0.1);
    let peer2 = NodePublicKey([1; 96]);
    map.insert(peer2, measurements.clone());

    let req = get_update_request_node(
        UpdateMethod::SubmitReputationMeasurements { measurements: map },
        keystore[0].node_secret_key,
        1,
    );

    if let Err(e) = run_transaction(vec![req], &update_socket).await {
        panic!("{e}");
    }

    let mut map = BTreeMap::new();
    let measurements = reputation::generate_reputation_measurements(&mut rng, 0.1);
    let peer1 = NodePublicKey([0; 96]);
    map.insert(peer1, measurements.clone());

    let measurements = reputation::generate_reputation_measurements(&mut rng, 0.1);
    let peer2 = NodePublicKey([1; 96]);
    map.insert(peer2, measurements.clone());

    let req = get_update_request_node(
        UpdateMethod::SubmitReputationMeasurements { measurements: map },
        keystore[1].node_secret_key,
        1,
    );

    if let Err(e) = run_transaction(vec![req], &update_socket).await {
        panic!("{e}");
    }

    // Change epoch so that rep scores will be calculated from the measurements.
    for (i, node) in keystore.iter().enumerate().take(required_signals) {
        // Not the prettiest solution but we have to keep track of the nonces somehow.
        let nonce = if i == 0 || i == 1 { 2 } else { 1 };
        let req = get_update_request_node(
            UpdateMethod::ChangeEpoch { epoch: 0 },
            node.node_secret_key,
            nonce,
        );
        run_transaction(vec![req], &update_socket).await.unwrap();
    }

    assert!(query_runner.get_reputation(&peer1).is_some());
    assert!(query_runner.get_reputation(&peer2).is_some());
}

#[test]
async fn test_supply_across_epoch() {
    let (committee, keystore) = get_genesis_committee(4);

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
        Some(committee),
    )
    .await;

    // get params for emission calculations
    let percentage_divisor: HpUfixed<18> = 100_u16.into();
    let supply_at_year_start: HpUfixed<18> = supply_at_genesis.into();
    let inflation: HpUfixed<18> = HpUfixed::from(max_inflation) / &percentage_divisor;
    let node_share = HpUfixed::from(node_part) / &percentage_divisor;
    let protocol_share = HpUfixed::from(protocol_part) / &percentage_divisor;
    let service_share = HpUfixed::from(service_part) / &percentage_divisor;

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_secret_key = NodeSecretKey::generate();

    // deposit FLK tokens and stake it
    deposit(
        10_000_u64.into(),
        Tokens::FLK,
        owner_secret_key,
        &update_socket,
        1,
    )
    .await;
    stake(
        10_000_u64.into(),
        node_secret_key.to_pk(),
        owner_secret_key,
        &update_socket,
        2,
    )
    .await;

    //every epoch supply increase similar for simplicity of the test
    let _node_1_usd = 0.1 * 10000_f64;

    // calculate emissions per unit
    let emissions_per_epoch: HpUfixed<18> = (&inflation * &supply_at_year_start) / &365.0.into();

    let mut supply = supply_at_year_start;

    // 365 epoch changes to see if the current supply and year start suppply are ok
    for i in 0..365 {
        let pod_10 = pod_request(node_secret_key, 10000, 0, i + 1);
        // run the delivery ack transaction
        if let Err(e) = run_transaction(vec![pod_10], &update_socket).await {
            panic!("{e}");
        }
        if let Err(err) = simple_epoch_change(i, &keystore, &update_socket, i + 1).await {
            panic!("error while changing epoch, {err}");
        }

        let supply_increase = &emissions_per_epoch * &node_share
            + &emissions_per_epoch * &protocol_share
            + &emissions_per_epoch * &service_share;
        let total_supply = query_runner.get_total_supply();
        let supply_year_start = query_runner.get_year_start_supply();
        supply += supply_increase;
        assert_eq!(total_supply, supply);
        if i == 364 {
            // the supply_year_start should update
            assert_eq!(total_supply, supply_year_start);
        }
    }
}

#[test]
async fn test_validate_txn() {
    let (committee, keystore) = get_genesis_committee(4);
    let mut genesis = Genesis::load().unwrap();
    genesis.committee = committee;
    let (update_socket, query_runner) = init_app(Some(Config {
        genesis: Some(genesis),
        mode: Mode::Test,
    }))
    .await;

    // Submit a ChangeEpoch transaction that will revert (EpochHasNotStarted) and ensure that the
    // `validate_txn` method of the query runner returns the same response as the update runner.
    let req = get_update_request_node(
        UpdateMethod::ChangeEpoch { epoch: 1 },
        keystore[0].node_secret_key,
        1,
    );
    let res = run_transaction(vec![req.clone()], &update_socket)
        .await
        .unwrap();
    let req = get_update_request_node(
        UpdateMethod::ChangeEpoch { epoch: 1 },
        keystore[0].node_secret_key,
        2,
    );
    assert_eq!(res.txn_receipts[0], query_runner.validate_txn(req));

    // Submit a ChangeEpoch transaction that will succeed and ensure that the
    // `validate_txn` method of the query runner returns the same response as the update runner.
    let req = get_update_request_node(
        UpdateMethod::ChangeEpoch { epoch: 0 },
        keystore[0].node_secret_key,
        2,
    );
    let res = run_transaction(vec![req], &update_socket).await.unwrap();
    let req = get_update_request_node(
        UpdateMethod::ChangeEpoch { epoch: 0 },
        keystore[1].node_secret_key,
        1,
    );
    assert_eq!(res.txn_receipts[0], query_runner.validate_txn(req));
}

#[test]
async fn test_is_valid_node() {
    let (update_socket, query_runner) = init_app(None).await;

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_secret_key = NodeSecretKey::generate();

    // Stake minimum required amount.
    let minimum_stake_amount = query_runner.get_staking_amount();
    deposit(
        minimum_stake_amount.into(),
        Tokens::FLK,
        owner_secret_key,
        &update_socket,
        1,
    )
    .await;
    stake(
        minimum_stake_amount.into(),
        node_secret_key.to_pk(),
        owner_secret_key,
        &update_socket,
        2,
    )
    .await;
    // Make sure that this node is a valid node.
    assert!(query_runner.is_valid_node(&node_secret_key.to_pk()));

    // Generate new keys for a different node.
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_secret_key = NodeSecretKey::generate();

    // Stake less than the minimum required amount.
    let less_than_minimum_skate_amount = minimum_stake_amount / 2;
    deposit(
        less_than_minimum_skate_amount.into(),
        Tokens::FLK,
        owner_secret_key,
        &update_socket,
        1,
    )
    .await;
    stake(
        less_than_minimum_skate_amount.into(),
        node_secret_key.to_pk(),
        owner_secret_key,
        &update_socket,
        2,
    )
    .await;
    // Make sure that this node is not a valid node.
    assert!(!query_runner.is_valid_node(&node_secret_key.to_pk()));
}

#[test]
async fn test_get_node_registry() {
    let (committee, keystore) = get_genesis_committee(4);
    let mut genesis = Genesis::load().unwrap();
    genesis.committee = committee;
    let (update_socket, query_runner) = init_app(Some(Config {
        genesis: Some(genesis),
        mode: Mode::Test,
    }))
    .await;

    let owner_secret_key1 = AccountOwnerSecretKey::generate();
    let node_secret_key1 = NodeSecretKey::generate();

    // Stake minimum required amount.
    let minimum_stake_amount = query_runner.get_staking_amount();
    deposit(
        minimum_stake_amount.into(),
        Tokens::FLK,
        owner_secret_key1,
        &update_socket,
        1,
    )
    .await;
    stake(
        minimum_stake_amount.into(),
        node_secret_key1.to_pk(),
        owner_secret_key1,
        &update_socket,
        2,
    )
    .await;

    // Generate new keys for a different node.
    let owner_secret_key2 = AccountOwnerSecretKey::generate();
    let node_secret_key2 = NodeSecretKey::generate();

    // Stake less than the minimum required amount.
    let less_than_minimum_skate_amount = minimum_stake_amount / 2;
    deposit(
        less_than_minimum_skate_amount.into(),
        Tokens::FLK,
        owner_secret_key2,
        &update_socket,
        1,
    )
    .await;
    stake(
        less_than_minimum_skate_amount.into(),
        node_secret_key2.to_pk(),
        owner_secret_key2,
        &update_socket,
        2,
    )
    .await;

    // Generate new keys for a different node.
    let owner_secret_key3 = AccountOwnerSecretKey::generate();
    let node_secret_key3 = NodeSecretKey::generate();

    // Stake minimum required amount.
    deposit(
        minimum_stake_amount.into(),
        Tokens::FLK,
        owner_secret_key3,
        &update_socket,
        1,
    )
    .await;
    stake(
        minimum_stake_amount.into(),
        node_secret_key3.to_pk(),
        owner_secret_key3,
        &update_socket,
        2,
    )
    .await;

    let valid_nodes = query_runner.get_node_registry();
    // We added two valid nodes, so the node registry should contain 2 nodes plus the committee.
    assert_eq!(valid_nodes.len(), 2 + keystore.len());
    let node_info1 = query_runner
        .get_node_info(&node_secret_key1.to_pk())
        .unwrap();
    // Node registry contains the first valid node
    assert!(valid_nodes.contains(&node_info1));
    let node_info2 = query_runner
        .get_node_info(&node_secret_key2.to_pk())
        .unwrap();
    // Node registry doesn't contain the invalid node
    assert!(!valid_nodes.contains(&node_info2));
    let node_info3 = query_runner
        .get_node_info(&node_secret_key3.to_pk())
        .unwrap();
    // Node registry contains the second valid node
    assert!(valid_nodes.contains(&node_info3));
}
