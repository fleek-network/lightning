use std::thread;
use std::time::Duration;

use affair::{Executor, TokioSpawn, Worker};
use anyhow::Result;
use fleek_crypto::{
    AccountOwnerSecretKey,
    ConsensusSecretKey,
    EthAddress,
    NodePublicKey,
    NodeSecretKey,
    SecretKey,
};
use hp_fixed::unsigned::HpUfixed;
use lightning_application::app::Application;
use lightning_application::config::{Config as AppConfig, Mode};
use lightning_application::genesis::{Genesis, GenesisAccount, GenesisNode};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{
    EpochInfo,
    NodeInfo,
    NodePorts,
    NodeServed,
    ProtocolParams,
    Staking,
    TotalServed,
    UpdateRequest,
};
use lightning_interfaces::{
    partial,
    ApplicationInterface,
    MempoolSocket,
    RpcInterface,
    SyncQueryRunnerInterface,
    WithStartAndShutdown,
};
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::{task, test};

use crate::config::Config as RpcConfig;
use crate::server::Rpc;

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
        TokioSpawn::spawn(MockWorker)
    }
}

impl Worker for MockWorker {
    type Request = UpdateRequest;
    type Response = ();

    fn handle(&mut self, _req: Self::Request) -> Self::Response {}
}

partial!(TestBinding {
    ApplicationInterface = Application<Self>;
    RpcInterface = Rpc<Self>;
});

fn init_rpc_without_consensus() -> Result<Rpc<TestBinding>> {
    let app = Application::<TestBinding>::init(AppConfig::default()).unwrap();

    let rpc = Rpc::<TestBinding>::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        app.sync_query(),
    )?;

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
    let mut rpc = init_rpc_without_consensus().unwrap();
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
        public_key: owner_public_key.into(),
        flk_balance: 1000,
        stables_balance: 0,
        bandwidth_balance: 0,
    });

    let app = Application::<TestBinding>::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30001;
    let mut rpc = Rpc::<TestBinding>::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner,
    )?;
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
            assert_eq!(HpUfixed::<18>::from(1_000_u32), success_response.result);
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
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();
    let node_secret_key = NodeSecretKey::generate();
    let node_public_key = node_secret_key.to_pk();
    let consensus_secret_key = ConsensusSecretKey::generate();
    let consensus_public_key = consensus_secret_key.to_pk();

    let mut genesis = Genesis::load().unwrap();

    let mut genesis_node = GenesisNode::new(
        owner_public_key.into(),
        node_public_key,
        "127.0.0.1".parse().unwrap(),
        consensus_public_key,
        "127.0.0.1".parse().unwrap(),
        node_public_key,
        NodePorts {
            primary: 48000,
            worker: 48101,
            mempool: 48102,
            rpc: 48103,
            pool: 48104,
            dht: 48105,
            handshake: 48106,
        },
        None,
        true,
    );
    // Init application service and store reputation score in application state.
    genesis_node.reputation = Some(46);
    genesis.node_info.push(genesis_node);

    let app = Application::<TestBinding>::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30002;
    let mut rpc = Rpc::<TestBinding>::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner,
    )?;
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
    let consensus_secret_key = ConsensusSecretKey::generate();
    let consensus_public_key = consensus_secret_key.to_pk();

    // Init application service and store node info in application state.
    let mut genesis = Genesis::load().unwrap();
    let staking = Staking {
        staked: 1_000_u32.into(),
        stake_locked_until: 365,
        locked: 0_u32.into(),
        locked_until: 0,
    };

    let node_info = GenesisNode::new(
        eth_address,
        node_public_key,
        "127.0.0.1".parse().unwrap(),
        consensus_public_key,
        "127.0.0.1".parse().unwrap(),
        node_public_key,
        NodePorts {
            primary: 38000,
            worker: 38101,
            mempool: 38102,
            rpc: 38103,
            pool: 38104,
            dht: 38105,
            handshake: 38106,
        },
        Some(staking),
        false,
    );

    genesis.node_info.push(node_info);

    let app = Application::<TestBinding>::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30003;
    let mut rpc = Rpc::<TestBinding>::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner,
    )?;
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
            //Parse the response as a successful response
            let success_response: RpcSuccessResponse<HpUfixed<18>> = serde_json::from_value(value)?;
            assert_eq!(HpUfixed::<18>::from(1_000_u32), success_response.result);
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
        public_key: owner_public_key.into(),
        flk_balance: 0,
        stables_balance: 200,
        bandwidth_balance: 0,
    });

    let app = Application::<TestBinding>::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30004;
    let mut rpc = Rpc::<TestBinding>::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner,
    )?;
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
            assert_eq!(HpUfixed::<6>::from(2_00_u32), success_response.result);
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
    let consensus_secret_key = ConsensusSecretKey::generate();
    let consensus_public_key = consensus_secret_key.to_pk();

    // Init application service and store node info in application state.
    let mut genesis = Genesis::load().unwrap();
    let staking = Staking {
        staked: 1_000_u32.into(),
        stake_locked_until: 365,
        locked: 0_u32.into(),
        locked_until: 0,
    };
    let node_info = GenesisNode::new(
        eth_address,
        node_public_key,
        "127.0.0.1".parse().unwrap(),
        consensus_public_key,
        "127.0.0.1".parse().unwrap(),
        node_public_key,
        NodePorts {
            primary: 48000,
            worker: 48101,
            mempool: 48102,
            rpc: 48103,
            pool: 48104,
            dht: 48105,
            handshake: 48106,
        },
        Some(staking),
        false,
    );

    genesis.node_info.push(node_info);

    let app = Application::<TestBinding>::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30005;
    let mut rpc = Rpc::<TestBinding>::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner,
    )?;
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
    let consensus_secret_key = ConsensusSecretKey::generate();
    let consensus_public_key = consensus_secret_key.to_pk();

    // Init application service and store node info in application state.
    let mut genesis = Genesis::load().unwrap();
    let staking = Staking {
        staked: 1_000_u32.into(),
        stake_locked_until: 365,
        locked: 0_u32.into(),
        locked_until: 2,
    };
    let node_info = GenesisNode::new(
        eth_address,
        node_public_key,
        "127.0.0.1".parse().unwrap(),
        consensus_public_key,
        "127.0.0.1".parse().unwrap(),
        node_public_key,
        NodePorts {
            primary: 48000,
            worker: 48101,
            mempool: 48102,
            rpc: 48103,
            pool: 48104,
            dht: 48105,
            handshake: 48106,
        },
        Some(staking),
        false,
    );

    genesis.node_info.push(node_info);

    let app = Application::<TestBinding>::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30006;
    let mut rpc = Rpc::<TestBinding>::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner,
    )?;
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
    let consensus_secret_key = ConsensusSecretKey::generate();
    let consensus_public_key = consensus_secret_key.to_pk();

    // Init application service and store node info in application state.
    let mut genesis = Genesis::load().unwrap();
    let staking = Staking {
        staked: 1_000_u32.into(),
        stake_locked_until: 365,
        locked: 500_u32.into(),
        locked_until: 2,
    };
    let node_info = GenesisNode::new(
        eth_address,
        node_public_key,
        "127.0.0.1".parse().unwrap(),
        consensus_public_key,
        "127.0.0.1".parse().unwrap(),
        node_public_key,
        NodePorts {
            primary: 48000,
            worker: 48101,
            mempool: 48102,
            rpc: 48103,
            pool: 48104,
            dht: 48105,
            handshake: 48106,
        },
        Some(staking),
        false,
    );

    genesis.node_info.push(node_info);

    let app = Application::<TestBinding>::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30007;
    let mut rpc = Rpc::<TestBinding>::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner,
    )?;
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
            assert_eq!(HpUfixed::<18>::from(500_u32), success_response.result);
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
        public_key: owner_public_key.into(),
        flk_balance: 0,
        stables_balance: 0,
        bandwidth_balance: 10_000,
    });

    let app = Application::<TestBinding>::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30008;
    let mut rpc = Rpc::<TestBinding>::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner,
    )?;
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
    let consensus_secret_key = ConsensusSecretKey::generate();
    let consensus_public_key = consensus_secret_key.to_pk();

    // Init application service and store node info in application state.
    let mut genesis = Genesis::load().unwrap();
    let staking = Staking {
        staked: 1_000_u32.into(),
        stake_locked_until: 365,
        locked: 500_u32.into(),
        locked_until: 2,
    };
    let node_info = GenesisNode::new(
        eth_address,
        node_public_key,
        "127.0.0.1".parse().unwrap(),
        consensus_public_key,
        "127.0.0.1".parse().unwrap(),
        node_public_key,
        NodePorts {
            primary: 48000,
            worker: 48101,
            mempool: 48102,
            rpc: 48103,
            pool: 48104,
            dht: 48105,
            handshake: 48106,
        },
        Some(staking),
        false,
    );

    genesis.node_info.push(node_info.clone());

    let app = Application::<TestBinding>::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30009;
    let mut rpc = Rpc::<TestBinding>::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner,
    )?;
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
            assert_eq!(Some(NodeInfo::from(&node_info)), success_response.result);
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
    let app = Application::<TestBinding>::init(AppConfig::default()).unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30010;
    let mut rpc = Rpc::<TestBinding>::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner.clone(),
    )?;
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
    let app = Application::<TestBinding>::init(AppConfig::default()).unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30011;
    let mut rpc = Rpc::<TestBinding>::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner.clone(),
    )?;
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
    let app = Application::<TestBinding>::init(AppConfig::default()).unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30012;
    let mut rpc = Rpc::<TestBinding>::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner.clone(),
    )?;
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
    let app = Application::<TestBinding>::init(AppConfig::default()).unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30013;
    let mut rpc = Rpc::<TestBinding>::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner.clone(),
    )?;
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
    let app = Application::<TestBinding>::init(AppConfig::default()).unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30014;
    let mut rpc = Rpc::<TestBinding>::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner.clone(),
    )?;
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
    let app = Application::<TestBinding>::init(AppConfig::default()).unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30015;
    let mut rpc = Rpc::<TestBinding>::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner.clone(),
    )?;
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
    let app = Application::<TestBinding>::init(AppConfig::default()).unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30016;
    let mut rpc = Rpc::<TestBinding>::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner.clone(),
    )?;
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
    let app = Application::<TestBinding>::init(AppConfig::default()).unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30017;
    let mut rpc = Rpc::<TestBinding>::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner.clone(),
    )?;
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

    let app = Application::<TestBinding>::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30018;
    let mut rpc = Rpc::<TestBinding>::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner,
    )?;
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
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();
    let node_secret_key = NodeSecretKey::generate();
    let node_public_key = node_secret_key.to_pk();
    let consensus_secret_key = ConsensusSecretKey::generate();
    let consensus_public_key = consensus_secret_key.to_pk();

    // Init application service and store total served in application state.
    let mut genesis = Genesis::load().unwrap();
    let mut genesis_node = GenesisNode::new(
        owner_public_key.into(),
        node_public_key,
        "127.0.0.1".parse().unwrap(),
        consensus_public_key,
        "127.0.0.1".parse().unwrap(),
        node_public_key,
        NodePorts {
            primary: 48000,
            worker: 48101,
            mempool: 48102,
            rpc: 48103,
            pool: 48104,
            dht: 48105,
            handshake: 48106,
        },
        None,
        true,
    );

    genesis_node.current_epoch_served = Some(NodeServed {
        served: vec![1000],
        ..Default::default()
    });
    genesis.node_info.push(genesis_node);

    let app = Application::<TestBinding>::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30019;
    let mut rpc = Rpc::<TestBinding>::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner,
    )?;
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
    let consensus_secret_key = ConsensusSecretKey::generate();
    let consensus_public_key = consensus_secret_key.to_pk();

    // Init application service and store node info in application state.
    let mut genesis = Genesis::load().unwrap();
    let staking = Staking {
        staked: genesis.min_stake.into(),
        stake_locked_until: 0,
        locked: 0_u32.into(),
        locked_until: 0,
    };
    let node_info = GenesisNode::new(
        eth_address,
        node_public_key,
        "127.0.0.1".parse().unwrap(),
        consensus_public_key,
        "127.0.0.1".parse().unwrap(),
        node_public_key,
        NodePorts {
            primary: 48000,
            worker: 48101,
            mempool: 48102,
            rpc: 48103,
            pool: 48104,
            dht: 48105,
            handshake: 48106,
        },
        Some(staking),
        false,
    );

    genesis.node_info.push(node_info);

    let app = Application::<TestBinding>::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30020;
    let mut rpc = Rpc::<TestBinding>::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner,
    )?;
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
    let consensus_secret_key = ConsensusSecretKey::generate();
    let consensus_public_key = consensus_secret_key.to_pk();

    // Init application service and store node info in application state.
    let mut genesis = Genesis::load().unwrap();
    let staking = Staking {
        staked: genesis.min_stake.into(),
        stake_locked_until: 0,
        locked: 0_u32.into(),
        locked_until: 0,
    };
    let node_info = GenesisNode::new(
        eth_address,
        node_public_key,
        "127.0.0.1".parse().unwrap(),
        consensus_public_key,
        "127.0.0.1".parse().unwrap(),
        node_public_key,
        NodePorts {
            primary: 48000,
            worker: 48101,
            mempool: 48102,
            rpc: 48103,
            pool: 48104,
            dht: 48105,
            handshake: 48106,
        },
        Some(staking),
        false,
    );

    genesis.node_info.push(node_info.clone());

    let committee_size =
        genesis.node_info.iter().fold(
            0,
            |acc, node| {
                if node.genesis_committee { acc + 1 } else { acc }
            },
        );

    let app = Application::<TestBinding>::init(AppConfig {
        genesis: Some(genesis),
        mode: Mode::Test,
    })
    .unwrap();
    let query_runner = app.sync_query();
    app.start().await;

    // Init rpc service
    let port = 30021;
    let mut rpc = Rpc::<TestBinding>::init(
        RpcConfig::default(),
        MockWorker::mempool_socket(),
        query_runner,
    )?;
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
            assert!(
                success_response
                    .result
                    .contains(&NodeInfo::from(&node_info))
            );
        } else {
            panic!("Rpc Error: {value}")
        }
    } else {
        panic!("Request failed with status: {}", response.status());
    }
    Ok(())
}
