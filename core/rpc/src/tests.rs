use std::{thread, time::Duration};

use affair::{Executor, TokioSpawn, Worker};
use anyhow::Result;
use fleek_crypto::{
    AccountOwnerSecretKey, EthAddress, NodeNetworkingSecretKey, NodePublicKey, NodeSecretKey,
    PublicKey, SecretKey,
};
use freek_application::{
    app::Application,
    config::{Config as AppConfig, Mode},
    genesis::{Genesis, GenesisAccount},
    query_runner::QueryRunner,
};
use freek_interfaces::{
    types::{
        EpochInfo, NodeInfo, NodeServed, ProtocolParams, Staking, TotalServed, UpdateRequest,
        Worker as NodeWorker,
    },
    ApplicationInterface, MempoolSocket, RpcInterface, SyncQueryRunnerInterface,
    WithStartAndShutdown,
};
use hp_fixed::unsigned::HpUfixed;
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
async fn test_rpc_get_flk_balance() -> Result<()> {
    // Create keys
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();
    let eth_address: EthAddress = owner_public_key.into();

    // Init application service
    let mut genesis = Genesis::load().unwrap();
    genesis.account.push(GenesisAccount {
        public_key: owner_public_key.to_base64(),
        flk_balance: 1000,
        stables_balance: 0,
        bandwidth_balance: 0,
    });

    let app = Application::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .await
    .unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30001;
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
        "method":"flk_get_flk_balance",
        "params": {"public_key": eth_address},
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse<HpUfixed<18>> = serde_json::from_value(value)?;
            assert_eq!(
                HpUfixed::<18>::new(1_000_u32.into()),
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
    let query_runner = app.sync_query();
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
    let query_runner = app.sync_query();
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
            let success_response: RpcSuccessResponse<HpUfixed<18>> = serde_json::from_value(value)?;
            assert_eq!(
                HpUfixed::<18>::new(1_000_u32.into()),
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
async fn test_rpc_get_stables_balance() -> Result<()> {
    // Create keys
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();
    let eth_address: EthAddress = owner_public_key.into();

    // Init application service
    let mut genesis = Genesis::load().unwrap();
    genesis.account.push(GenesisAccount {
        public_key: owner_public_key.to_base64(),
        flk_balance: 0,
        stables_balance: 200,
        bandwidth_balance: 0,
    });

    let app = Application::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .await
    .unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30004;
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
        "method":"flk_get_stables_balance",
        "params": {"public_key": eth_address},
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse<HpUfixed<6>> = serde_json::from_value(value)?;
            assert_eq!(HpUfixed::<6>::new(2_00_u32.into()), success_response.result);
        } else {
            panic!("Rpc Error: {value}")
        }
    } else {
        panic!("Request failed with status: {}", response.status());
    }
    Ok(())
}

#[test]
async fn test_rpc_get_stake_locked_until() -> Result<()> {
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
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30005;
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
        "method":"flk_get_stake_locked_until",
        "params": {"public_key": node_public_key},
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse<u64> = serde_json::from_value(value)?;
            assert_eq!(365, success_response.result);
        } else {
            panic!("Rpc Error: {value}")
        }
    } else {
        panic!("Request failed with status: {}", response.status());
    }
    Ok(())
}

#[test]
async fn test_rpc_get_locked_time() -> Result<()> {
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
        locked_until: 2,
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
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30006;
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
        "method":"flk_get_locked_time",
        "params": {"public_key": node_public_key},
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse<u64> = serde_json::from_value(value)?;
            assert_eq!(2, success_response.result);
        } else {
            panic!("Rpc Error: {value}")
        }
    } else {
        panic!("Request failed with status: {}", response.status());
    }
    Ok(())
}

#[test]
async fn test_rpc_get_locked() -> Result<()> {
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
        locked: 500_u32.into(),
        locked_until: 2,
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
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30007;
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
        "method":"flk_get_locked",
        "params": {"public_key": node_public_key},
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse<HpUfixed<18>> = serde_json::from_value(value)?;
            assert_eq!(HpUfixed::<18>::new(500_u32.into()), success_response.result);
        } else {
            panic!("Rpc Error: {value}")
        }
    } else {
        panic!("Request failed with status: {}", response.status());
    }
    Ok(())
}

#[test]
async fn test_rpc_get_bandwidth_balance() -> Result<()> {
    // Create keys
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();
    let eth_address: EthAddress = owner_public_key.into();

    // Init application service
    let mut genesis = Genesis::load().unwrap();
    genesis.account.push(GenesisAccount {
        public_key: owner_public_key.to_base64(),
        flk_balance: 0,
        stables_balance: 0,
        bandwidth_balance: 10_000,
    });

    let app = Application::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .await
    .unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30008;
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
        "method":"flk_get_bandwidth_balance",
        "params": {"public_key": eth_address},
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse<u128> = serde_json::from_value(value)?;
            assert_eq!(10_000, success_response.result);
        } else {
            panic!("Rpc Error: {value}")
        }
    } else {
        panic!("Request failed with status: {}", response.status());
    }
    Ok(())
}

#[test]
async fn test_rpc_get_node_info() -> Result<()> {
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
        locked: 500_u32.into(),
        locked_until: 2,
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
        .insert(node_public_key.to_base64(), node_info.clone());

    let app = Application::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .await
    .unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30009;
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
        "method":"flk_get_node_info",
        "params": {"public_key": node_public_key},
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse<Option<NodeInfo>> =
                serde_json::from_value(value)?;
            assert_eq!(Some(node_info), success_response.result);
        } else {
            panic!("Rpc Error: {value}")
        }
    } else {
        panic!("Request failed with status: {}", response.status());
    }
    Ok(())
}

#[test]
async fn test_rpc_get_staking_amount() -> Result<()> {
    let app = Application::init(AppConfig::default()).await.unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30010;
    let mut rpc = Rpc::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner.clone(),
    )
    .await?;
    rpc.config.port = port;

    task::spawn(async move {
        rpc.start().await;
    });
    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_staking_amount",
        "params":[],
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse<u128> = serde_json::from_value(value)?;
            assert_eq!(query_runner.get_staking_amount(), success_response.result);
        } else {
            panic!("Rpc Error: {value}")
        }
    } else {
        panic!("Request failed with status: {}", response.status());
    }

    Ok(())
}

#[test]
async fn test_rpc_get_committee_members() -> Result<()> {
    let app = Application::init(AppConfig::default()).await.unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30011;
    let mut rpc = Rpc::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner.clone(),
    )
    .await?;
    rpc.config.port = port;

    task::spawn(async move {
        rpc.start().await;
    });
    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_committee_members",
        "params":[],
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse<Vec<NodePublicKey>> =
                serde_json::from_value(value)?;
            assert_eq!(
                query_runner.get_committee_members(),
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
async fn test_rpc_get_epoch() -> Result<()> {
    let app = Application::init(AppConfig::default()).await.unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30012;
    let mut rpc = Rpc::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner.clone(),
    )
    .await?;
    rpc.config.port = port;

    task::spawn(async move {
        rpc.start().await;
    });
    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_epoch",
        "params":[],
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse<u64> = serde_json::from_value(value)?;
            assert_eq!(query_runner.get_epoch(), success_response.result);
        } else {
            panic!("Rpc Error: {value}")
        }
    } else {
        panic!("Request failed with status: {}", response.status());
    }

    Ok(())
}

#[test]
async fn test_rpc_get_epoch_info() -> Result<()> {
    let app = Application::init(AppConfig::default()).await.unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30013;
    let mut rpc = Rpc::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner.clone(),
    )
    .await?;
    rpc.config.port = port;

    task::spawn(async move {
        rpc.start().await;
    });
    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_epoch_info",
        "params":[],
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse<EpochInfo> = serde_json::from_value(value)?;
            assert_eq!(query_runner.get_epoch_info(), success_response.result);
        } else {
            panic!("Rpc Error: {value}")
        }
    } else {
        panic!("Request failed with status: {}", response.status());
    }

    Ok(())
}

#[test]
async fn test_rpc_get_total_supply() -> Result<()> {
    let app = Application::init(AppConfig::default()).await.unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30014;
    let mut rpc = Rpc::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner.clone(),
    )
    .await?;
    rpc.config.port = port;

    task::spawn(async move {
        rpc.start().await;
    });
    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_total_supply",
        "params":[],
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse<HpUfixed<18>> = serde_json::from_value(value)?;
            assert_eq!(query_runner.get_total_supply(), success_response.result);
        } else {
            panic!("Rpc Error: {value}")
        }
    } else {
        panic!("Request failed with status: {}", response.status());
    }

    Ok(())
}

#[test]
async fn test_rpc_get_year_start_supply() -> Result<()> {
    let app = Application::init(AppConfig::default()).await.unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30015;
    let mut rpc = Rpc::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner.clone(),
    )
    .await?;
    rpc.config.port = port;

    task::spawn(async move {
        rpc.start().await;
    });
    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_year_start_supply",
        "params":[],
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse<HpUfixed<18>> = serde_json::from_value(value)?;
            assert_eq!(
                query_runner.get_year_start_supply(),
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
async fn test_rpc_get_protocol_fund_address() -> Result<()> {
    let app = Application::init(AppConfig::default()).await.unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30016;
    let mut rpc = Rpc::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner.clone(),
    )
    .await?;
    rpc.config.port = port;

    task::spawn(async move {
        rpc.start().await;
    });
    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_protocol_fund_address",
        "params":[],
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse<EthAddress> = serde_json::from_value(value)?;
            assert_eq!(
                query_runner.get_protocol_fund_address(),
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
async fn test_rpc_get_protocol_params() -> Result<()> {
    let app = Application::init(AppConfig::default()).await.unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30017;
    let mut rpc = Rpc::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner.clone(),
    )
    .await?;
    rpc.config.port = port;

    task::spawn(async move {
        rpc.start().await;
    });
    wait_for_server_start(port).await?;

    let params = ProtocolParams::LockTime;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_protocol_params",
        "params": params,
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse<u128> = serde_json::from_value(value)?;
            assert_eq!(
                query_runner.get_protocol_params(params),
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
async fn test_rpc_get_total_served() -> Result<()> {
    // Init application service and store total served in application state.
    let mut genesis = Genesis::load().unwrap();

    let total_served = TotalServed {
        served: vec![1000],
        reward_pool: 1_000_u32.into(),
    };
    genesis.total_served.insert(0, total_served.clone());

    let app = Application::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .await
    .unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30018;
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
        "method":"flk_get_total_served",
        "params": 0,
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse<TotalServed> = serde_json::from_value(value)?;
            assert_eq!(total_served, success_response.result);
        } else {
            panic!("Rpc Error: {value}")
        }
    } else {
        panic!("Request failed with status: {}", response.status());
    }
    Ok(())
}

#[test]
async fn test_rpc_get_node_served() -> Result<()> {
    // Init application service and store total served in application state.
    let mut genesis = Genesis::load().unwrap();

    let node_secret_key = NodeSecretKey::generate();
    let node_public_key = node_secret_key.to_pk();
    genesis.current_epoch_served.insert(
        node_public_key.to_base64(),
        NodeServed {
            served: vec![1000],
            ..Default::default()
        },
    );

    let app = Application::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .await
    .unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30019;
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
        "method":"flk_get_node_served",
        "params": {"public_key": node_public_key},
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse<NodeServed> = serde_json::from_value(value)?;
            assert_eq!(vec![1000], success_response.result.served);
        } else {
            panic!("Rpc Error: {value}")
        }
    } else {
        panic!("Request failed with status: {}", response.status());
    }
    Ok(())
}

#[test]
async fn test_rpc_is_valid_node() -> Result<()> {
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
        staked: genesis.min_stake.into(),
        stake_locked_until: 0,
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
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30020;
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
        "method":"flk_is_valid_node",
        "params": {"public_key": node_public_key},
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse<bool> = serde_json::from_value(value)?;
            assert!(success_response.result);
        } else {
            panic!("Rpc Error: {value}")
        }
    } else {
        panic!("Request failed with status: {}", response.status());
    }
    Ok(())
}

#[test]
async fn test_rpc_get_node_registry() -> Result<()> {
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
        staked: genesis.min_stake.into(),
        stake_locked_until: 0,
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
        .insert(node_public_key.to_base64(), node_info.clone());

    let committee_size = genesis.committee.len();

    let app = Application::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .await
    .unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30021;
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
        "method":"flk_get_node_registry",
        "params": [],
        "id":1,
    });

    let response = make_request(port, req.to_string()).await?;

    if response.status().is_success() {
        let value: Value = response.json().await?;
        if value.get("result").is_some() {
            // Parse the response as a successful response
            let success_response: RpcSuccessResponse<Vec<NodeInfo>> =
                serde_json::from_value(value)?;
            assert_eq!(success_response.result.len(), committee_size + 1);
            assert!(success_response.result.contains(&node_info));
        } else {
            panic!("Rpc Error: {value}")
        }
    } else {
        panic!("Request failed with status: {}", response.status());
    }
    Ok(())
}
