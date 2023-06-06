use affair::Socket;
use draco_interfaces::{
    application::{ExecutionEngineSocket, QuerySocket},
    types::{
        Block, ExecutionData, ExecutionError, NodeInfo, ProofOfConsensus, QueryMethod,
        QueryRequest, Tokens, TransactionResponse, UpdateMethod, UpdatePayload, UpdateRequest,
    },
    ApplicationInterface,
};
use fastcrypto::{bls12381::min_sig::BLS12381PublicKey, traits::EncodeDecodeBase64};
use fleek_crypto::{
    AccountOwnerPublicKey, AccountOwnerSignature, NodePublicKey, NodeSignature,
    TransactionSignature,
};
use tokio::test;

use crate::{app::Application, config::Config, genesis::Genesis};

const ACCOUNT_ONE: AccountOwnerPublicKey = AccountOwnerPublicKey([0; 32]);
const NODE_ONE: &str = "k7XAk/1z4rXf1QHyMPHZ1cgyeX2T3bsCCopNpFV6v8hInZfjyti79w3raEa3YwFADM2BnX+/o49k1HQjKZIYlGDszEZ/zUaK3kn3MfT5BEWkKgP+TFMPJoBxenV33XEZ";

// Init the app and return the execution engine socket that would go to narwhal and the query socket
// that could go to anyone
async fn init_app() -> (ExecutionEngineSocket, QuerySocket) {
    let app = Application::init(Config {}).await.unwrap();

    (app.transaction_executor(), app.query_socket())
}

#[test]
async fn test_genesis() {
    // Init application + get the query and update socket
    let (_, query_socket) = init_app().await;

    // Get the genesis paramaters plus the initial committee
    let (genesis, genesis_committee) = get_genesis();

    // For every member of the genesis committee they should have an initial stake of the min stake
    // Query to make sure that holds true
    for node in genesis_committee {
        let query = get_query(QueryMethod::Staked {
            node: node.public_key,
        });
        let res = run_query(query, &query_socket).await;
        assert_eq!(
            TransactionResponse::Success(ExecutionData::UInt(genesis.min_stake as u128)),
            res
        );
    }
}

#[test]
async fn test_epoch_change() {
    // Init application + get the query and update socket
    let (update_socket, query_socket) = init_app().await;
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
    let query = get_query(QueryMethod::CurrentEpochInfo);
    let res = run_query(query.clone(), &query_socket).await;
    if let TransactionResponse::Success(ExecutionData::EpochInfo {
        committee: _,
        epoch,
        epoch_end: _,
    }) = res
    {
        assert_eq!(epoch, 0);
    }

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
    let res = run_query(query, &query_socket).await;
    if let TransactionResponse::Success(ExecutionData::EpochInfo {
        committee: _,
        epoch,
        epoch_end: _,
    }) = res
    {
        assert_eq!(epoch, 1);
    }
}

#[test]
async fn test_stake() {
    let (update_socket, query_socket) = init_app().await;
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
    let query = get_query(QueryMethod::FLK {
        public_key: ACCOUNT_ONE,
    });
    let res = run_query(query, &query_socket).await;
    assert_eq!(
        TransactionResponse::Success(ExecutionData::UInt(2_000)),
        res
    );

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
    let query = get_query(QueryMethod::Staked {
        node: node_public_key,
    });
    let res = run_query(query.clone(), &query_socket).await;
    assert_eq!(TransactionResponse::Success(ExecutionData::UInt(1000)), res);

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
    let res = run_query(query.clone(), &query_socket).await;
    assert_eq!(TransactionResponse::Success(ExecutionData::UInt(2000)), res);

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

    let locked_query = get_query(QueryMethod::Locked {
        public_key: node_public_key,
    });
    let locked_until_query = get_query(QueryMethod::LockedUntil {
        public_key: node_public_key,
    });
    let res_staked = run_query(query, &query_socket).await;
    let res_locked = run_query(locked_query, &query_socket).await;
    let res_locked_until = run_query(locked_until_query, &query_socket).await;
    assert_eq!(
        TransactionResponse::Success(ExecutionData::UInt(1000)),
        res_staked
    );
    assert_eq!(
        TransactionResponse::Success(ExecutionData::UInt(1000)),
        res_locked
    );
    // Since this test starts at epoch 0 locked_until will be == lock_time
    assert_eq!(
        TransactionResponse::Success(ExecutionData::UInt(genesis.lock_time as u128)),
        res_locked_until
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

fn get_genesis() -> (Genesis, Vec<NodeInfo>) {
    let genesis = Genesis::load().unwrap();

    (
        genesis.clone(),
        genesis.committee.iter().map(|node| node.into()).collect(),
    )
}

fn get_query(method: QueryMethod) -> QueryRequest {
    QueryRequest {
        sender: ACCOUNT_ONE.into(),
        query: method,
    }
}

async fn run_query(
    req: QueryRequest,
    socket: &Socket<QueryRequest, Vec<u8>>,
) -> TransactionResponse {
    let res = socket.run(req).await.unwrap();
    bincode::deserialize(&res).unwrap()
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
