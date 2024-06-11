use std::net::SocketAddr;
use std::time::Duration;

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
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use lightning_application::app::Application;
use lightning_application::config::Config as AppConfig;
use lightning_application::genesis::{Genesis, GenesisAccount, GenesisNode};
use lightning_application::query_runner::QueryRunner;
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore::config::Config as BlockstoreConfig;
use lightning_blockstore_server::BlockstoreServer;
use lightning_fetcher::fetcher::Fetcher;
use lightning_indexer::Indexer;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    Blake3Hash,
    EpochInfo,
    Event,
    Metadata,
    NodeInfo,
    NodePorts,
    NodeServed,
    ProtocolParams,
    Staking,
    TotalServed,
    Value,
};
use lightning_interfaces::PagingParams;
use lightning_notifier::Notifier;
use lightning_origin_demuxer::OriginDemuxer;
use lightning_pool::PoolProvider;
use lightning_rep_collector::ReputationAggregator;
use lightning_signer::Signer;
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_utils::application::QueryRunnerExt;
use lightning_utils::rpc as utils;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tempfile::{tempdir, TempDir};

use crate::api::FleekApiClient;
use crate::config::Config as RpcConfig;
use crate::Rpc;

#[derive(Serialize, Deserialize, Debug)]
struct RpcSuccessResponse<T> {
    jsonrpc: String,
    id: usize,
    result: T,
}

partial!(TestBinding {
    ConfigProviderInterface = JsonConfigProvider;
    ApplicationInterface = Application<Self>;
    FetcherInterface = Fetcher<Self>;
    RpcInterface = Rpc<Self>;
    BlockstoreInterface = Blockstore<Self>;
    BlockstoreServerInterface = BlockstoreServer<Self>;
    OriginProviderInterface = OriginDemuxer<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    SignerInterface = Signer<Self>;
    NotifierInterface = Notifier<Self>;
    PoolInterface = PoolProvider<Self>;
    ReputationAggregatorInterface = ReputationAggregator<Self>;
    IndexerInterface = Indexer<Self>;
});

struct TestNode {
    inner: Node<TestBinding>,
}

impl TestNode {
    async fn shutdown(mut self) {
        self.inner.shutdown().await
    }
    fn rpc(&self) -> fdi::Ref<Rpc<TestBinding>> {
        self.inner.provider.get()
    }
    fn query_runner(&self) -> fdi::Ref<QueryRunner> {
        self.inner.provider.get()
    }
    fn blockstore(&self) -> fdi::Ref<Blockstore<TestBinding>> {
        self.inner.provider.get()
    }
}

async fn init_rpc(temp_dir: &TempDir, genesis: Option<Genesis>, rpc_port: u16) -> TestNode {
    let app_config = genesis
        .map(|gen| AppConfig {
            genesis: Some(gen),
            ..AppConfig::test()
        })
        .unwrap_or(AppConfig::test());

    let node = Node::<TestBinding>::init_with_provider(
        fdi::Provider::default().with(
            JsonConfigProvider::default()
                .with::<Rpc<TestBinding>>(RpcConfig::default_with_port(rpc_port))
                .with::<Application<TestBinding>>(app_config)
                .with::<Blockstore<TestBinding>>(BlockstoreConfig {
                    root: temp_dir.path().join("blockstore").try_into().unwrap(),
                }),
        ),
    )
    .expect("failed to initialize node");
    node.start().await;

    TestNode { inner: node }
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
            Ok(res) => {
                if res.status().is_success() {
                    println!("Server is ready");
                    break;
                } else {
                    println!(
                        "Server is not ready yet status: {}, res: {}",
                        res.status(),
                        res.text().await?
                    );
                    retries -= 1;
                    // Delay between retries
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            },
            Err(e) => {
                println!("Server Error: {}", e);
                retries -= 1;
                // Delay between retries
                tokio::time::sleep(Duration::from_secs(1)).await;
            },
        }
    }

    if retries > 0 {
        Ok(())
    } else {
        panic!("Server did not become ready within the specified time");
    }
}

fn client(url: SocketAddr) -> HttpClient {
    HttpClientBuilder::default()
        .build(format!("http://{}", url))
        .expect("client to build")
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_ping() -> Result<()> {
    let temp_dir = tempdir()?;

    let port = 30000;
    let node = init_rpc(&temp_dir, None, port).await;

    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_ping",
        "params":[],
        "id":1,
    });

    let response =
        utils::make_request(format!("http://127.0.0.1:{port}/AHHHHHH"), req.to_string()).await?;

    if response.status().is_success() {
        let response_body = response.text().await?;
        println!("Response body: {response_body}");
    } else {
        panic!("Request failed with status: {}", response.status());
    }

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_flk_balance() -> Result<()> {
    let temp_dir = tempdir()?;

    // Create keys
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();
    let eth_address: EthAddress = owner_public_key.into();

    // Init application service
    let mut genesis = Genesis::default();
    genesis.account.push(GenesisAccount {
        public_key: owner_public_key.into(),
        flk_balance: 1000u64.into(),
        stables_balance: 0,
        bandwidth_balance: 0,
    });

    let port = 30001;
    let node = init_rpc(&temp_dir, Some(genesis), port).await;

    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_flk_balance",
        "params": {"public_key": eth_address},
        "id":1,
    });

    let client = Client::new();
    let response = utils::rpc_request::<HpUfixed<18>>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        req.to_string(),
    )
    .await?;
    assert_eq!(HpUfixed::<18>::from(1_000_u32), response.result);

    node.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_reputation() -> Result<()> {
    let temp_dir = tempdir()?;

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();
    let node_secret_key = NodeSecretKey::generate();
    let node_public_key = node_secret_key.to_pk();
    let consensus_secret_key = ConsensusSecretKey::generate();
    let consensus_public_key = consensus_secret_key.to_pk();

    let mut genesis = Genesis::default();
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
            pinger: 48106,
            // not used in TestBinding, so defaults are fine
            handshake: Default::default(),
        },
        None,
        true,
    );
    // Init application service and store reputation score in application state.
    genesis_node.reputation = Some(46);
    genesis.node_info.push(genesis_node);

    let port = 30002;
    let node = init_rpc(&temp_dir, Some(genesis), port).await;

    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_reputation",
        "params": {"public_key": node_public_key},
        "id":1,
    });

    let client = Client::new();
    let response = utils::rpc_request::<Option<u8>>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        req.to_string(),
    )
    .await?;
    assert_eq!(Some(46), response.result);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_staked() -> Result<()> {
    let temp_dir = tempdir()?;

    // Create keys
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();
    let eth_address = owner_public_key.into();
    let node_secret_key = NodeSecretKey::generate();
    let node_public_key = node_secret_key.to_pk();
    let consensus_secret_key = ConsensusSecretKey::generate();
    let consensus_public_key = consensus_secret_key.to_pk();

    // Init application service and store node info in application state.
    let mut genesis = Genesis::default();
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
            pinger: 38106,
            handshake: Default::default(),
        },
        Some(staking),
        false,
    );
    genesis.node_info.push(node_info);

    let port = 30003;
    let node = init_rpc(&temp_dir, Some(genesis), port).await;
    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_staked",
        "params": {"public_key": node_public_key},
        "id":1,
    });

    let client = Client::new();
    let response = utils::rpc_request::<HpUfixed<18>>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        req.to_string(),
    )
    .await?;
    assert_eq!(HpUfixed::<18>::from(1_000_u32), response.result);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_stables_balance() -> Result<()> {
    let temp_dir = tempdir()?;

    // Create keys
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();
    let eth_address: EthAddress = owner_public_key.into();

    // Init application service
    let mut genesis = Genesis::default();
    genesis.account.push(GenesisAccount {
        public_key: owner_public_key.into(),
        flk_balance: 0u64.into(),
        stables_balance: 200,
        bandwidth_balance: 0,
    });

    let port = 30004;
    let node = init_rpc(&temp_dir, Some(genesis), port).await;

    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_stables_balance",
        "params": {"public_key": eth_address},
        "id":1,
    });

    let client = Client::new();
    let response = utils::rpc_request::<HpUfixed<6>>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        req.to_string(),
    )
    .await?;
    assert_eq!(HpUfixed::<6>::from(2_00_u32), response.result);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_stake_locked_until() -> Result<()> {
    let temp_dir = tempdir()?;

    // Create keys
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();
    let eth_address = owner_public_key.into();
    let node_secret_key = NodeSecretKey::generate();
    let node_public_key = node_secret_key.to_pk();
    let consensus_secret_key = ConsensusSecretKey::generate();
    let consensus_public_key = consensus_secret_key.to_pk();

    // Init application service and store node info in application state.
    let mut genesis = Genesis::default();
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
            pinger: 48106,
            handshake: Default::default(),
        },
        Some(staking),
        false,
    );

    genesis.node_info.push(node_info);

    let port = 30005;
    let node = init_rpc(&temp_dir, Some(genesis), port).await;

    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_stake_locked_until",
        "params": {"public_key": node_public_key},
        "id":1,
    });

    let client = Client::new();
    let response = utils::rpc_request::<u64>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        req.to_string(),
    )
    .await?;
    assert_eq!(365, response.result);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_locked_time() -> Result<()> {
    let temp_dir = tempdir()?;

    // Create keys
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();
    let eth_address = owner_public_key.into();
    let node_secret_key = NodeSecretKey::generate();
    let node_public_key = node_secret_key.to_pk();
    let consensus_secret_key = ConsensusSecretKey::generate();
    let consensus_public_key = consensus_secret_key.to_pk();

    // Init application service and store node info in application state.
    let mut genesis = Genesis::default();
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
            pinger: 48106,
            handshake: Default::default(),
        },
        Some(staking),
        false,
    );
    genesis.node_info.push(node_info);

    let port = 30006;
    let node = init_rpc(&temp_dir, Some(genesis), port).await;

    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_locked_time",
        "params": {"public_key": node_public_key},
        "id":1,
    });

    let client = Client::new();
    let response = utils::rpc_request::<u64>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        req.to_string(),
    )
    .await?;
    assert_eq!(2, response.result);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_locked() -> Result<()> {
    let temp_dir = tempdir()?;

    // Create keys
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();
    let eth_address = owner_public_key.into();
    let node_secret_key = NodeSecretKey::generate();
    let node_public_key = node_secret_key.to_pk();
    let consensus_secret_key = ConsensusSecretKey::generate();
    let consensus_public_key = consensus_secret_key.to_pk();

    // Init application service and store node info in application state.
    let mut genesis = Genesis::default();
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
            pinger: 48106,
            handshake: Default::default(),
        },
        Some(staking),
        false,
    );
    genesis.node_info.push(node_info);

    let port = 30007;
    let node = init_rpc(&temp_dir, Some(genesis), port).await;

    wait_for_server_start(port).await?;

    let client = client(node.rpc().config.addr());

    let res = crate::api::FleekApiClient::get_locked(&client, node_public_key, None).await?;
    assert_eq!(HpUfixed::<18>::from(500_u32), res);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_bandwidth_balance() -> Result<()> {
    let temp_dir = tempdir()?;

    // Create keys
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();
    let eth_address: EthAddress = owner_public_key.into();

    // Init application service
    let mut genesis = Genesis::default();
    genesis.account.push(GenesisAccount {
        public_key: owner_public_key.into(),
        flk_balance: 0u64.into(),
        stables_balance: 0,
        bandwidth_balance: 10_000,
    });

    let port = 30008;
    let node = init_rpc(&temp_dir, Some(genesis), port).await;

    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_bandwidth_balance",
        "params": {"public_key": eth_address},
        "id":1,
    });

    let client = Client::new();
    let response = utils::rpc_request::<u128>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        req.to_string(),
    )
    .await?;
    assert_eq!(10_000, response.result);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_node_info() -> Result<()> {
    let temp_dir = tempdir()?;

    // Create keys
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();
    let eth_address = owner_public_key.into();
    let node_secret_key = NodeSecretKey::generate();
    let node_public_key = node_secret_key.to_pk();
    let consensus_secret_key = ConsensusSecretKey::generate();
    let consensus_public_key = consensus_secret_key.to_pk();

    // Init application service and store node info in application state.
    let mut genesis = Genesis::default();
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
            pinger: 48106,
            handshake: Default::default(),
        },
        Some(staking),
        false,
    );
    genesis.node_info.push(node_info.clone());

    let port = 30009;
    let node = init_rpc(&temp_dir, Some(genesis), port).await;

    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_node_info",
        "params": {"public_key": node_public_key},
        "id":1,
    });

    let client = Client::new();
    let response = utils::rpc_request::<Option<NodeInfo>>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        req.to_string(),
    )
    .await?;
    assert_eq!(Some(NodeInfo::from(&node_info)), response.result);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_staking_amount() -> Result<()> {
    let temp_dir = tempdir()?;

    let port = 30010;
    let node = init_rpc(&temp_dir, None, port).await;

    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_staking_amount",
        "params":[],
        "id":1,
    });

    let client = Client::new();
    let response = utils::rpc_request::<u128>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        req.to_string(),
    )
    .await?;
    assert_eq!(node.query_runner().get_staking_amount(), response.result);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_committee_members() -> Result<()> {
    let temp_dir = tempdir()?;

    let port = 30011;
    let node = init_rpc(&temp_dir, None, port).await;

    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_committee_members",
        "params":[],
        "id":1,
    });

    let client = Client::new();
    let response = utils::rpc_request::<Vec<NodePublicKey>>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        req.to_string(),
    )
    .await?;
    assert_eq!(node.query_runner().get_committee_members(), response.result);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_epoch() -> Result<()> {
    let temp_dir = tempdir()?;

    let port = 30012;
    let node = init_rpc(&temp_dir, None, port).await;

    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_epoch",
        "params":[],
        "id":1,
    });

    let client = Client::new();
    let response = utils::rpc_request::<u64>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        req.to_string(),
    )
    .await?;
    assert_eq!(node.query_runner().get_current_epoch(), response.result);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_epoch_info() -> Result<()> {
    let temp_dir = tempdir()?;

    let port = 30013;
    let node = init_rpc(&temp_dir, None, port).await;

    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_epoch_info",
        "params":[],
        "id":1,
    });

    let client = Client::new();
    let response = utils::rpc_request::<EpochInfo>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        req.to_string(),
    )
    .await?;
    assert_eq!(node.query_runner().get_epoch_info(), response.result);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_total_supply() -> Result<()> {
    let temp_dir = tempdir()?;

    let port = 30014;
    let node = init_rpc(&temp_dir, None, port).await;

    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_total_supply",
        "params":[],
        "id":1,
    });

    let client = Client::new();
    let response = utils::rpc_request::<HpUfixed<18>>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        req.to_string(),
    )
    .await?;

    let total_supply = match node.query_runner().get_metadata(&Metadata::TotalSupply) {
        Some(Value::HpUfixed(s)) => s,
        _ => panic!("TotalSupply is set genesis and should never be empty"),
    };

    assert_eq!(total_supply, response.result);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_year_start_supply() -> Result<()> {
    let temp_dir = tempdir()?;

    let port = 30015;
    let node = init_rpc(&temp_dir, None, port).await;

    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_year_start_supply",
        "params":[],
        "id":1,
    });

    let client = Client::new();
    let response = utils::rpc_request::<HpUfixed<18>>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        req.to_string(),
    )
    .await?;

    let supply_year_start = match node.query_runner().get_metadata(&Metadata::SupplyYearStart) {
        Some(Value::HpUfixed(s)) => s,
        _ => panic!("SupplyYearStart is set genesis and should never be empty"),
    };

    assert_eq!(supply_year_start, response.result);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_protocol_fund_address() -> Result<()> {
    let temp_dir = tempdir()?;

    let port = 30016;
    let node = init_rpc(&temp_dir, None, port).await;

    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_protocol_fund_address",
        "params":[],
        "id":1,
    });

    let client = Client::new();
    let response = utils::rpc_request::<EthAddress>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        req.to_string(),
    )
    .await?;

    let protocol_account = match node
        .query_runner()
        .get_metadata(&Metadata::ProtocolFundAddress)
    {
        Some(Value::AccountPublicKey(s)) => s,
        _ => panic!("AccountPublicKey is set genesis and should never be empty"),
    };

    assert_eq!(protocol_account, response.result);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_protocol_params() -> Result<()> {
    let temp_dir = tempdir()?;

    let port = 30017;
    let node = init_rpc(&temp_dir, None, port).await;

    wait_for_server_start(port).await?;

    let params = ProtocolParams::LockTime;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_protocol_params",
        "params": {"protocol_params": params},
        "id":1,
    });

    let client = Client::new();
    let response = utils::rpc_request::<u128>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        req.to_string(),
    )
    .await?;
    assert_eq!(
        node.query_runner().get_protocol_param(&params).unwrap(),
        response.result
    );

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_total_served() -> Result<()> {
    let temp_dir = tempdir()?;

    // Init application service and store total served in application state.
    let mut genesis = Genesis::default();
    let total_served = TotalServed {
        served: vec![1000],
        reward_pool: 1_000_u32.into(),
    };
    genesis.total_served.insert(0, total_served.clone());

    let port = 30018;
    let node = init_rpc(&temp_dir, Some(genesis), port).await;

    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_total_served",
        "params": { "epoch": 0 },
        "id":1,
    });

    let client = Client::new();
    let response = utils::rpc_request::<TotalServed>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        req.to_string(),
    )
    .await?;
    assert_eq!(total_served, response.result);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_node_served() -> Result<()> {
    let temp_dir = tempdir()?;

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();
    let node_secret_key = NodeSecretKey::generate();
    let node_public_key = node_secret_key.to_pk();
    let consensus_secret_key = ConsensusSecretKey::generate();
    let consensus_public_key = consensus_secret_key.to_pk();

    // Init application service and store total served in application state.
    let mut genesis = Genesis::default();
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
            pinger: 48106,
            handshake: Default::default(),
        },
        None,
        true,
    );
    genesis_node.current_epoch_served = Some(NodeServed {
        served: vec![1000],
        ..Default::default()
    });
    genesis.node_info.push(genesis_node);

    let port = 30019;
    let node = init_rpc(&temp_dir, Some(genesis), port).await;

    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_node_served",
        "params": {"public_key": node_public_key},
        "id":1,
    });

    let client = Client::new();
    let response = utils::rpc_request::<NodeServed>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        req.to_string(),
    )
    .await?;
    assert_eq!(vec![1000], response.result.served);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_is_valid_node() -> Result<()> {
    let temp_dir = tempdir()?;

    // Create keys
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();
    let eth_address = owner_public_key.into();
    let node_secret_key = NodeSecretKey::generate();
    let node_public_key = node_secret_key.to_pk();
    let consensus_secret_key = ConsensusSecretKey::generate();
    let consensus_public_key = consensus_secret_key.to_pk();

    // Init application service and store node info in application state.
    let mut genesis = Genesis::default();
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
            pinger: 48106,
            handshake: Default::default(),
        },
        Some(staking),
        false,
    );
    genesis.node_info.push(node_info);

    let port = 30020;
    let node = init_rpc(&temp_dir, Some(genesis), port).await;
    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_is_valid_node",
        "params": {"public_key": node_public_key},
        "id":1,
    });

    let client = Client::new();
    let response = utils::rpc_request::<bool>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        req.to_string(),
    )
    .await?;
    assert!(response.result);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_node_registry() -> Result<()> {
    let temp_dir = tempdir()?;

    // Create keys
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();
    let eth_address = owner_public_key.into();
    let node_secret_key = NodeSecretKey::generate();
    let node_public_key = node_secret_key.to_pk();
    let consensus_secret_key = ConsensusSecretKey::generate();
    let consensus_public_key = consensus_secret_key.to_pk();

    // Init application service and store node info in application state.
    let mut genesis = Genesis::default();
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
            pinger: 48106,
            handshake: Default::default(),
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

    let port = 30021;
    let node = init_rpc(&temp_dir, Some(genesis), port).await;

    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_node_registry",
        "params": [],
        "id":1,
    });

    let client = Client::new();
    let response = utils::rpc_request::<Vec<NodeInfo>>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        req.to_string(),
    )
    .await?;
    assert_eq!(response.result.len(), committee_size + 1);
    assert!(response.result.contains(&NodeInfo::from(&node_info)));

    let req = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_node_registry",
        "params": PagingParams { ignore_stake: true, start: committee_size as u32, limit: 10 },
        "id":1,
    });

    let response = utils::rpc_request::<Vec<NodeInfo>>(
        &client,
        format!("http://127.0.0.1:{port}/rpc/v0"),
        req.to_string(),
    )
    .await?;
    assert!(response.result.contains(&NodeInfo::from(&node_info)));

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_admin_rpc_store() -> Result<()> {
    let temp_dir = tempdir()?;

    let port = 30022;
    let node = init_rpc(&temp_dir, None, port).await;

    wait_for_server_start(port).await?;

    let req = json!({
        "jsonrpc": "2.0",
        "method": "admin_store",
        "params": { "path": "../test-utils/files/index.ts" },
        "id": 1,
    });

    let client = Client::new();
    let response = utils::rpc_request::<Blake3Hash>(
        &client,
        format!("http://127.0.0.1:{port}/admin"),
        req.to_string(),
    )
    .await?;

    let expected_content: Vec<u8> = std::fs::read("../test-utils/files/index.ts").unwrap();
    let stored_content = node
        .blockstore()
        .read_all_to_vec(&response.result)
        .await
        .unwrap();
    assert_eq!(expected_content, stored_content);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
// #[traced_test]
async fn test_rpc_events() -> Result<()> {
    let temp_dir = tempdir()?;

    // Create keys
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();

    // Init application service
    let mut genesis = Genesis::default();
    genesis.account.push(GenesisAccount {
        public_key: owner_public_key.into(),
        flk_balance: 1000u64.into(),
        stables_balance: 0,
        bandwidth_balance: 0,
    });

    let port = 30023;
    let node = init_rpc(&temp_dir, Some(genesis), port).await;

    wait_for_server_start(port).await?;

    let sender = node.rpc().event_tx();

    let client = jsonrpsee::ws_client::WsClientBuilder::default()
        .build(&format!("ws://127.0.0.1:{port}/rpc/v0"))
        .await?;

    let mut sub = FleekApiClient::handle_subscription(&client, None).await?;

    let event = Event::transfer(
        EthAddress::from([0; 20]),
        EthAddress::from([1; 20]),
        EthAddress::from([2; 20]),
        HpUfixed::<18>::from(10_u16),
    );

    sender
        .send(vec![event.clone()])
        .await
        .expect("can send event");

    assert_eq!(sub.next().await.expect("An event from the sub")?, event);

    node.shutdown().await;

    Ok(())
}
