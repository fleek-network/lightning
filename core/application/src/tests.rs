use std::vec;

use affair::Socket;
use anyhow::{anyhow, Result};
use big_decimal::BigDecimal;
use draco_interfaces::{
    application::ExecutionEngineSocket,
    types::{
        Block, ExecutionError, NodeInfo, ProofOfConsensus, ProtocolParams, Tokens, TotalServed,
        TransactionResponse, UpdateMethod, UpdatePayload, UpdateRequest,
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
use tokio::test;

use crate::{app::Application, config::Config, genesis::Genesis, query_runner::QueryRunner};

const ACCOUNT_ONE: AccountOwnerPublicKey = AccountOwnerPublicKey([0; 32]);
const NODE_ONE: &str = "k7XAk/1z4rXf1QHyMPHZ1cgyeX2T3bsCCopNpFV6v8hInZfjyti79w3raEa3YwFADM2BnX+/o49k1HQjKZIYlGDszEZ/zUaK3kn3MfT5BEWkKgP+TFMPJoBxenV33XEZ";

// Init the app and return the execution engine socket that would go to narwhal and the query socket
// that could go to anyone
async fn init_app() -> (ExecutionEngineSocket, QueryRunner) {
    let app = Application::init(Config::default()).await.unwrap();

    (app.transaction_executor(), app.sync_query())
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
    update_socket: Socket<Block, BlockExecutionResponse>,
) {
    // Deposit some FLK into account 1
    let req = get_update_request_account(
        UpdateMethod::Deposit {
            proof: ProofOfConsensus {},
            token,
            amount,
        },
        ACCOUNT_ONE,
    );
    run_transaction(vec![req], &update_socket).await.unwrap();
}

async fn stake(
    amount: BigDecimal<18>,
    update_socket: Socket<Block, BlockExecutionResponse>,
    node_public_key: NodePublicKey,
) {
    // Now try with the correct details for a new node
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
}

#[test]
async fn test_genesis() {
    // Init application + get the query and update socket
    let (_, query_runner) = init_app().await;

    // Get the genesis paramaters plus the initial committee
    let (genesis, genesis_committee) = get_genesis();

    // For every member of the genesis committee they should have an initial stake of the min stake
    // Query to make sure that holds true
    for node in genesis_committee {
        let balance = query_runner.get_staked(&node.public_key);
        assert_eq!(BigDecimal::<18>::from(genesis.min_stake), balance);
    }
}

#[test]
async fn test_epoch_change() {
    // Init application + get the query and update socket
    let (update_socket, query_runner) = init_app().await;
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
    let (update_socket, query_runner) = init_app().await;
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
        genesis.lock_time
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
    let (update_socket, query_runner) = init_app().await;
    let node_public_key: NodePublicKey = BLS12381PublicKey::decode_base64(NODE_ONE)
        .unwrap()
        .pubkey
        .to_bytes()
        .into();

    deposit(1_000_u64.into(), Tokens::FLK, update_socket.clone()).await;
    assert_eq!(query_runner.get_flk_balance(&ACCOUNT_ONE), 1_000_u64.into());

    stake(1_000_u64.into(), update_socket.clone(), node_public_key).await;
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
    let (update_socket, query_runner) = init_app().await;

    // use a node from a genesis committee for testing
    let node_key = "l0Jel6KEFG7H6sV2nWKOQxDaMKWMeiUBqK5VHKcStWrLPHAANRB+dt7gp0jQ7ooxEaI7ukOQZk6U5vcL7ESHA1J/iAWQ7YNO/ZCvR1pfWfcTNBONIzeiUWAN+iyKfV10";
    let node_public_key: NodePublicKey = BLS12381PublicKey::decode_base64(node_key)
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
#[ignore = "uses some 'todo' implementations"]
async fn test_distribute_rewards() {
    let (update_socket, query_runner) = init_app().await;
    let (_, genesis_committee) = get_genesis();
    // use a node from a genesis committee for testing
    let owner_key = "EfP5ha4KNRu/qkfIuF3lWK7GPeP5IqPKP8esnM0mo2s=";
    let owner: AccountOwnerPublicKey = AccountOwnerPublicKey(
        Ed25519PublicKey::decode_base64(owner_key)
            .unwrap()
            .0
            .to_bytes(),
    );

    let node_key = "l0Jel6KEFG7H6sV2nWKOQxDaMKWMeiUBqK5VHKcStWrLPHAANRB+dt7gp0jQ7ooxEaI7ukOQZk6U5vcL7ESHA1J/iAWQ7YNO/ZCvR1pfWfcTNBONIzeiUWAN+iyKfV10";
    let node_public_key: NodePublicKey = BLS12381PublicKey::decode_base64(node_key)
        .unwrap()
        .pubkey
        .to_bytes()
        .into();

    let node_key_2 = "qipezx5pzmPFWICevMx+SL5+bIjG4yw3A9ieYKKwf2wTEvK0gMRYOln9+KmbNRB3FRbVQBLuCEWIHT0V9GxATT9VeJ+HT88vh/B/6dj7CbWBdWbZ4QXzo0q+uyGchopl";
    let node_public_key_2: NodePublicKey = BLS12381PublicKey::decode_base64(node_key_2)
        .unwrap()
        .pubkey
        .to_bytes()
        .into();

    // set the current supply to some number

    // submit pods for usage
    let pod_10 = pod_request(node_public_key, 10000, 0);
    let pod11 = pod_request(node_public_key, 5000, 1);
    let pod_21 = pod_request(node_public_key_2, 5000, 1);

    // run the delivery ack transaction
    if let Err(e) = run_transaction(vec![pod_10, pod11, pod_21], &update_socket).await {
        panic!("{e}");
    }

    // call epoch change that will trigger distribute rewards
    let required_signals = 2 * genesis_committee.len() / 3 + 1;
    // make call epoch change for 2/3rd committe members
    for (index, node) in genesis_committee.iter().enumerate().take(required_signals) {
        let req = get_update_request_node(UpdateMethod::ChangeEpoch { epoch: 0 }, node.public_key);
        let res = run_transaction(vec![req], &update_socket).await.unwrap();
        // check epoch change
        if index == required_signals - 1 {
            assert!(res.change_epoch);
        }
    }

    // check account balances for FLK and usdc
    let max_boost = query_runner.get_protocol_params(ProtocolParams::MaxBoost);
    let supply_for_inflation = query_runner.get_year_start_supply();
    let max_inflation = query_runner.get_protocol_params(ProtocolParams::MaxInflation);

    let emissions_per_day = supply_for_inflation.mul(&(max_inflation / (365 * 100)).into());
    let min_emission = emissions_per_day.div(&max_boost.into());

    let node_1_usd = 0.1 * 10000_f64 + 0.2 * 5000_f64;
    let node_2_usd = 0.2 * 5000_f64;

    let node_1_proportion = node_1_usd / (node_2_usd + node_1_usd);
    // Todo: add locking when locking is added to distribute_rewards
    let node_1_flk: BigDecimal<18> = min_emission.mul(&node_1_proportion.into());

    let flk_balance = query_runner.get_flk_balance(&owner);
    let stables_balance = query_runner.get_stables_balance(&owner);

    assert_eq!(flk_balance, node_1_flk);
    assert_eq!(stables_balance, node_1_usd.into());
}
