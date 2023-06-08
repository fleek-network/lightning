use std::{thread, time::Duration};

use affair::{Executor, TokioSpawn, Worker};
use anyhow::Result;
use draco_application::{app::Application, config::Config as AppConfig, query_runner::QueryRunner};
use draco_interfaces::{
    types::{Block, ProofOfConsensus, Tokens, UpdateMethod, UpdatePayload, UpdateRequest},
    ApplicationInterface, ExecutionEngineSocket, MempoolSocket, RpcInterface, WithStartAndShutdown,
};
use fleek_crypto::{AccountOwnerPublicKey, AccountOwnerSignature, TransactionSignature};
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::{task, test};

use crate::{config::Config as RpcConfig, server::Rpc};

const ACCOUNT_ONE: AccountOwnerPublicKey = AccountOwnerPublicKey([0; 32]);

#[derive(Serialize, Deserialize, Debug)]
struct RpcSuccessResponse {
    jsonrpc: String,
    id: usize,
    result: u128,
}

/// get mempool socket for test cases that do not require consensus
#[derive(Default)]
pub struct MockWorker;

impl MockWorker {
    fn mempool_socket() -> MempoolSocket {
        TokioSpawn::spawn(MockWorker::default())
    }
}

impl Worker for MockWorker {
    type Request = UpdateRequest;
    type Response = ();

    fn handle(&mut self, _req: Self::Request) -> Self::Response {}
}

async fn init_rpc_without_consensus() -> Result<Rpc<QueryRunner>> {
    let app = Application::init(AppConfig {}).await.unwrap();

    let rpc = Rpc::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        app.sync_query(),
    )
    .await?;

    Ok(rpc)
}

async fn init_rpc_with_execution_socket() -> Result<(Rpc<QueryRunner>, ExecutionEngineSocket)> {
    let app = Application::init(AppConfig {}).await.unwrap();

    let rpc = Rpc::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        app.sync_query(),
    )
    .await?;

    Ok((rpc, app.transaction_executor()))
}

async fn wait_for_server_start(port: u16) -> Result<()> {
    let client = Client::new();
    let mut retries = 10; // Maximum number of retries

    while retries > 0 {
        let response = client
            .get(format!("http://127.0.0.1:{port}/health"))
            .send()
            .await;
        match response {
            Ok(res) if res.status().is_success() => {
                println!("Server is ready!");
                break;
            },
            _ => {
                retries -= 1;
                // Delay between retries
                thread::sleep(Duration::from_secs(1));
            },
        }
    }

    if retries > 0 {
        Ok(())
    } else {
        panic!("Server did not become ready within the specified time");
    }
}

async fn make_request(port: u16, req: String) -> Result<Response> {
    let client = Client::new();
    Ok(client
        .post(format!("http://127.0.0.1:{port}/rpc/v0"))
        .header("Content-Type", "application/json")
        .body(req)
        .send()
        .await?)
}
#[test]
async fn test_rpc_ping() -> Result<()> {
    let port = 30000;
    let mut rpc = init_rpc_without_consensus().await.unwrap();
    rpc.config.port = port;
    task::spawn(async move {
        rpc.start().await;
    });

    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_ping",
        "params":[],
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let response_body = response.text().await?;
        println!("Response body: {response_body}");
    } else {
        panic!("Request failed with status: {}", response.status());
    }

    Ok(())
}

#[test]
async fn test_rpc_get_balance() -> Result<()> {
    let port = 30001;
    let (mut rpc, update_socket) = init_rpc_with_execution_socket().await.unwrap();
    rpc.config.port = port;
    task::spawn(async move {
        rpc.start().await;
    });
    wait_for_server_start(port).await?;

    let deposit_method = UpdateMethod::Deposit {
        proof: ProofOfConsensus {},
        token: Tokens::FLK,
        amount: 1_000,
    };

    // deposit FLK to test get balance
    let update = UpdateRequest {
        sender: ACCOUNT_ONE.into(),
        signature: TransactionSignature::AccountOwner(AccountOwnerSignature),
        payload: UpdatePayload {
            nonce: 0,
            method: deposit_method,
        },
    };

    update_socket
        .run(Block {
            transactions: [update].into(),
        })
        .await
        .unwrap();

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_balance",
        "params": {"public_key": ACCOUNT_ONE},
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse = serde_json::from_value(value)?;
            assert_eq!(1000, success_response.result);
        } else {
            panic!("Rpc Error: {value}")
        }
    } else {
        panic!("Request failed with status: {}", response.status());
    }
    Ok(())
}
