use std::vec;

use draco_interfaces::{
    application::ExecutionEngineSocket,
    types::{
        Block, ExecutionError, NodeInfo, ProofOfConsensus, Tokens, TotalServed,
        TransactionResponse, UpdateMethod, UpdatePayload, UpdateRequest,
    },
    ApplicationInterface, DeliveryAcknowledgment, SyncQueryRunnerInterface,
};
use fastcrypto::{bls12381::min_sig::BLS12381PublicKey, traits::EncodeDecodeBase64};
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
    let app = Application::init(Config {}).await.unwrap();

    (app.transaction_executor(), app.sync_query())
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
        assert_eq!(genesis.min_stake as u128, balance);
    }
}

#[test]
async fn test_epoch_change() {
    // Init application + get the query and update socket
    let (update_socket, query_runner) = init_app().await;
    let (_, genesis_committee) = get_genesis();

    let required_signals = genesis_committee.len() / 2 + 1;

    // Have (required_signals - 1) say they are ready to change epoch
    // make sure the epoch doesnt change each time someone signals
    for node in genesis_committee.iter().take(required_signals - 1) {
        let req = get_update_request_node(UpdateMethod::ChangeEpoch, node.public_key);

        let res = update_socket
            .run(Block {
                transactions: [req].into(),
            })
            .await
            .unwrap();
        // Make sure epoch didnt change
        assert!(!res.change_epoch);
    }
    // check that the current epoch is still 0
    assert_eq!(query_runner.get_epoch_info().epoch, 0);

    // Have the last needed committee member signal the epoch change and make sure it changes
    let req = get_update_request_node(
        UpdateMethod::ChangeEpoch,
        genesis_committee[required_signals].public_key,
    );
    let res = update_socket
        .run(Block {
            transactions: [req].into(),
        })
        .await
        .unwrap();
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
            amount: 1_000,
        },
        ACCOUNT_ONE,
    );
    // Put 2 of the transaction in the block just to also test block exucution a bit
    update_socket
        .run(Block {
            transactions: [update.clone(), update].into(),
        })
        .await
        .unwrap();

    // check that he has 2_000 flk balance
    assert_eq!(query_runner.get_flk_balance(&ACCOUNT_ONE), 2_000);

    // Test staking on a new node

    // First check that trying to stake without providing all the node info reverts
    let update = get_update_request_account(
        UpdateMethod::Stake {
            amount: 100,
            node_public_key,
            node_network_key: None,
            node_domain: None,
            worker_public_key: None,
            worker_domain: None,
            worker_mempool_address: None,
        },
        ACCOUNT_ONE,
    );
    let res = update_socket
        .run(Block {
            transactions: [update].into(),
        })
        .await
        .unwrap();

    assert_eq!(
        TransactionResponse::Revert(ExecutionError::InsufficientNodeDetails),
        res.txn_receipts[0]
    );

    // Now try with the correct details for a new node
    let update = get_update_request_account(
        UpdateMethod::Stake {
            amount: 1000,
            node_public_key,
            node_network_key: Some([0; 32].into()),
            node_domain: Some("/ip4/127.0.0.1/udp/38000".to_string()),
            worker_public_key: Some([0; 32].into()),
            worker_domain: Some("/ip4/127.0.0.1/udp/38000".to_string()),
            worker_mempool_address: Some("/ip4/127.0.0.1/udp/38000".to_string()),
        },
        ACCOUNT_ONE,
    );
    let res = update_socket
        .run(Block {
            transactions: [update].into(),
        })
        .await
        .unwrap()
        .txn_receipts[0]
        .clone();
    if let TransactionResponse::Revert(error) = res {
        panic!("Stake reverted: {error:?}");
    }

    // Query the new node and make sure he has the proper stake
    assert_eq!(query_runner.get_staked(&node_public_key), 1000);

    // Stake 1000 more but since it is not a new node we should be able to leave the optional
    // paramaters out without a revert
    let update = get_update_request_account(
        UpdateMethod::Stake {
            amount: 1000,
            node_public_key,
            node_network_key: None,
            node_domain: None,
            worker_public_key: None,
            worker_domain: None,
            worker_mempool_address: None,
        },
        ACCOUNT_ONE,
    );
    let res = update_socket
        .run(Block {
            transactions: [update].into(),
        })
        .await
        .unwrap()
        .txn_receipts[0]
        .clone();
    if let TransactionResponse::Revert(error) = res {
        panic!("Stake reverted: {error:?}");
    }

    // Node should now have 2_000 stake
    assert_eq!(query_runner.get_staked(&node_public_key), 2000);

    // Now test unstake and make sure it moves the tokens to locked status
    let update = get_update_request_account(
        UpdateMethod::Unstake {
            amount: 1000,
            node: node_public_key,
        },
        ACCOUNT_ONE,
    );
    update_socket
        .run(Block {
            transactions: [update].into(),
        })
        .await
        .unwrap();

    // Check that his locked is 1000 and his remaining stake is 1000
    assert_eq!(query_runner.get_staked(&node_public_key), 1000);
    assert_eq!(query_runner.get_locked(&node_public_key), 1000);
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
    let res = update_socket
        .run(Block {
            transactions: [update].into(),
        })
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
async fn test_pod_without_proof() {
    let (update_socket, query_runner) = init_app().await;

    // use a node from a genesis committee for testing
    let node_key = "l0Jel6KEFG7H6sV2nWKOQxDaMKWMeiUBqK5VHKcStWrLPHAANRB+dt7gp0jQ7ooxEaI7ukOQZk6U5vcL7ESHA1J/iAWQ7YNO/ZCvR1pfWfcTNBONIzeiUWAN+iyKfV10";
    let node_public_key: NodePublicKey = BLS12381PublicKey::decode_base64(node_key)
        .unwrap()
        .pubkey
        .to_bytes()
        .into();

    let bandwidth_pod = get_update_request_node(
        UpdateMethod::SubmitDeliveryAcknowledgmentAggregation {
            commodity: 1000, // 1000 units
            service_id: 0,   // service 0 serving bandwidth
            proofs: vec![DeliveryAcknowledgment::default()],
            metadata: None,
        },
        node_public_key,
    );

    let compute_pod = get_update_request_node(
        UpdateMethod::SubmitDeliveryAcknowledgmentAggregation {
            commodity: 2000, // 2000 units
            service_id: 1,   // service 1 serving compute
            proofs: vec![DeliveryAcknowledgment::default()],
            metadata: None,
        },
        node_public_key,
    );
    // run the delivery ack transaction
    update_socket
        .run(Block {
            transactions: [bandwidth_pod, compute_pod].into(),
        })
        .await
        .unwrap();
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
fn get_genesis() -> (Genesis, Vec<NodeInfo>) {
    let genesis = Genesis::load().unwrap();

    (
        genesis.clone(),
        genesis.committee.iter().map(|node| node.into()).collect(),
    )
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
