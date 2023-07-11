use std::{thread, time::Duration};

use affair::{Executor, TokioSpawn, Worker};
use anyhow::Result;
use draco_application::{
    app::Application,
    config::{Config as AppConfig, Mode},
    genesis::Genesis,
    query_runner::QueryRunner,
};
use draco_interfaces::{
    types::{
        Block, NodeInfo, ProofOfConsensus, Staking, Tokens, UpdateMethod, UpdatePayload,
        UpdateRequest, Worker as NodeWorker,
    },
    ApplicationInterface, ExecutionEngineSocket, MempoolSocket, RpcInterface, ToDigest,
    WithStartAndShutdown,
};
use fleek_crypto::{
    AccountOwnerSecretKey, EthAddress, NodeNetworkingSecretKey, NodeSecretKey, PublicKey, SecretKey,
};
use hp_float::unsigned::HpUfloat;
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::{task, test};

use crate::{config::Config as RpcConfig, server::Rpc};

#[derive(Serialize, Deserialize, Debug)]
struct RpcSuccessResponse<T> {
    jsonrpc: String,
    id: usize,
    result: T,
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
    let app = Application::init(AppConfig::default()).await.unwrap();

    let rpc = Rpc::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        app.sync_query(),
    )
    .await?;

    Ok(rpc)
}

async fn init_rpc_with_execution_socket() -> Result<(Rpc<QueryRunner>, ExecutionEngineSocket)> {
    let app = Application::init(AppConfig::default()).await.unwrap();

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
        amount: HpUfloat::<18>::new(1_000_u32.into()),
    };

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let eth_address: EthAddress = owner_secret_key.to_pk().into();

    let payload = UpdatePayload {
        nonce: 1,
        method: deposit_method,
    };
    let digest = payload.to_digest();
    let signature = owner_secret_key.sign(&digest);

    // deposit FLK to test get balance
    let update = UpdateRequest {
        sender: owner_secret_key.to_pk().into(),
        signature: signature.into(),
        payload,
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
        "params": {"public_key": eth_address},
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse<HpUfloat<18>> = serde_json::from_value(value)?;
            assert_eq!(
                HpUfloat::<18>::new(1_000_u32.into()),
                success_response.result
            );
        } else {
            panic!("Rpc Error: {value}")
        }
    } else {
        panic!("Request failed with status: {}", response.status());
    }
    Ok(())
}

#[test]
async fn test_rpc_get_reputation() -> Result<()> {
    // Create keys
    let node_secret_key = NodeSecretKey::generate();
    let node_public_key = node_secret_key.to_pk();

    // Init application service and store reputation score in application state.
    let mut genesis = Genesis::load().unwrap();
    genesis.rep_scores.insert(node_public_key.to_base64(), 46);

    let app = Application::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .await
    .unwrap();
    let (_, query_runner) = (app.transaction_executor(), app.sync_query());
    app.start().await;

    // Init rpc service
    let port = 30002;
    let mut rpc = Rpc::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner,
    )
    .await?;
    rpc.config.port = port;

    task::spawn(async move {
        rpc.start().await;
    });
    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_reputation",
        "params": {"public_key": node_public_key},
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse<Option<u8>> = serde_json::from_value(value)?;
            assert_eq!(Some(46), success_response.result);
        } else {
            panic!("Rpc Error: {value}")
        }
    } else {
        panic!("Request failed with status: {}", response.status());
    }
    Ok(())
}

#[test]
async fn test_rpc_get_staked() -> Result<()> {
    // Create keys
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();
    let eth_address = owner_public_key.into();
    let node_secret_key = NodeSecretKey::generate();
    let node_public_key = node_secret_key.to_pk();
    let network_secret_key = NodeNetworkingSecretKey::generate();
    let network_public_key = network_secret_key.to_pk();

    // Init application service and store node info in application state.
    let mut genesis = Genesis::load().unwrap();
    let staking = Staking {
        staked: 1_000_u32.into(),
        stake_locked_until: 365,
        locked: 0_u32.into(),
        locked_until: 0,
    };
    let node_info = NodeInfo {
        owner: eth_address,
        public_key: node_public_key,
        network_key: network_public_key,
        staked_since: 1,
        stake: staking,
        domain: "/ip4/127.0.0.1/udp/38000".parse().unwrap(),
        workers: vec![NodeWorker {
            public_key: network_public_key,
            address: "/ip4/127.0.0.1/udp/38101/http".parse().unwrap(),
            mempool: "/ip4/127.0.0.1/tcp/38102/http".parse().unwrap(),
        }],
        nonce: 0,
    };

    genesis
        .node_info
        .insert(node_public_key.to_base64(), node_info);

    let app = Application::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .await
    .unwrap();
    let (_, query_runner) = (app.transaction_executor(), app.sync_query());
    app.start().await;

    // Init rpc service
    let port = 30003;
    let mut rpc = Rpc::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner,
    )
    .await?;
    rpc.config.port = port;

    task::spawn(async move {
        rpc.start().await;
    });
    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_staked",
        "params": {"public_key": node_public_key},
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse<HpUfloat<18>> = serde_json::from_value(value)?;
            assert_eq!(
                HpUfloat::<18>::new(1_000_u32.into()),
                success_response.result
            );
        } else {
            panic!("Rpc Error: {value}")
        }
    } else {
        panic!("Request failed with status: {}", response.status());
    }
    Ok(())
}
