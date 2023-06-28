use std::{collections::BTreeMap, time::Duration};

use affair::Socket;
use anyhow::{anyhow, Result};
use draco_application::{app::Application, config::Config as AppConfig};
use draco_interfaces::{
    application::{ApplicationInterface, BlockExecutionResponse},
    common::WithStartAndShutdown,
    consensus::ConsensusInterface,
    signer::SignerInterface,
    types::{
        Block, ProofOfConsensus, Tokens, TransactionResponse, UpdateMethod, UpdatePayload,
        UpdateRequest,
    },
    SyncQueryRunnerInterface,
};
use draco_test_utils::consensus::{Config as ConsensusConfig, MockConsensus};
use fleek_crypto::{AccountOwnerPublicKey, AccountOwnerSignature, NodePublicKey};
use hp_float::unsigned::HpUfloat;

use crate::{config::Config, Signer};

// TODO: copied from application tests, this should be in test-utils.
fn get_update_request_account(
    method: UpdateMethod,
    sender: AccountOwnerPublicKey,
) -> UpdateRequest {
    // TODO: sign the thing
    UpdateRequest {
        sender: sender.into(),
        signature: AccountOwnerSignature([0; 64]).into(),
        payload: UpdatePayload { nonce: 0, method },
    }
}

async fn deposit(
    amount: HpUfloat<18>,
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

async fn stake(
    amount: HpUfloat<18>,
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

#[tokio::test]
async fn test_send_two_txs_in_a_row() {
    let signer_config = Config::default();
    let mut signer = Signer::init(signer_config).await.unwrap();
    let signer_socket = signer.get_socket();

    let app = Application::init(AppConfig::default()).await.unwrap();
    app.start().await;

    let (update_socket, query_runner) = (app.transaction_executor(), app.sync_query());

    let consensus_config = ConsensusConfig {
        min_ordering_time: 0,
        max_ordering_time: 2,
        probability_transaction_lost: 0.0,
    };
    let consensus = MockConsensus::init(
        consensus_config,
        &signer,
        update_socket.clone(),
        query_runner.clone(),
    )
    .await
    .unwrap();

    signer.provide_mempool(consensus.mempool());
    signer.provide_query_runner(query_runner.clone());
    signer.start().await;
    consensus.start().await;

    // Make sure that node info is created before sending the transaction.
    let account = AccountOwnerPublicKey([0; 33]);
    deposit(1_000_u64.into(), Tokens::FLK, account, &update_socket).await;
    stake(
        1_000_u64.into(),
        signer.get_bls_pk(),
        AccountOwnerPublicKey([0; 33]),
        &update_socket,
    )
    .await;

    // Send two transactions to the signer.
    let update_method = UpdateMethod::SubmitReputationMeasurements {
        measurements: BTreeMap::new(),
    };
    signer_socket.run(update_method).await.unwrap();
    let update_method = UpdateMethod::SubmitReputationMeasurements {
        measurements: BTreeMap::new(),
    };
    signer_socket.run(update_method).await.unwrap();

    // Each transaction will take at most 2 seconds to get ordered.
    // Therefore, after 5 seconds, the nonce should be 2.
    tokio::time::sleep(Duration::from_secs(5)).await;
    let new_nonce = query_runner
        .get_node_info(&signer.get_bls_pk())
        .unwrap()
        .nonce;
    assert_eq!(new_nonce, 2);
}
