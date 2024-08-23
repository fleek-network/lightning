use std::net::SocketAddr;
use std::time::Duration;

use anyhow::Result;
use fleek_crypto::{
    AccountOwnerSecretKey,
    ConsensusSecretKey,
    EthAddress,
    NodeSecretKey,
    SecretKey,
};
use hp_fixed::unsigned::HpUfixed;
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use lightning_application::app::Application;
use lightning_application::config::Config as AppConfig;
use lightning_application::genesis::{Genesis, GenesisAccount, GenesisNode, GenesisNodeServed};
use lightning_application::state::{ApplicationMerklizeProvider, QueryRunner};
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore::config::Config as BlockstoreConfig;
use lightning_blockstore_server::BlockstoreServer;
use lightning_fetcher::fetcher::Fetcher;
use lightning_indexer::Indexer;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    Event,
    Metadata,
    NodeInfo,
    NodePorts,
    ProtocolParams,
    Staking,
    TotalServed,
    Value,
};
use lightning_notifier::Notifier;
use lightning_origin_demuxer::OriginDemuxer;
use lightning_pool::PoolProvider;
use lightning_rep_collector::ReputationAggregator;
use lightning_signer::Signer;
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_types::{AccountInfo, FirewallConfig, StateProofKey, StateProofValue};
use lightning_utils::application::QueryRunnerExt;
use merklize::StateProof;
use reqwest::Client;
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};
use tempfile::{tempdir, TempDir};

use crate::api::{AdminApiClient, FleekApiClient, RpcClient};
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
}

async fn init_rpc(temp_dir: &TempDir, genesis_path: ResolvedPathBuf, rpc_port: u16) -> TestNode {
    let app_config = AppConfig::test(genesis_path);

    let rpc_config = RpcConfig {
        hmac_secret_dir: Some(temp_dir.path().to_path_buf()),
        firewall: FirewallConfig::none(format!("rpc-{}", rpc_port)),
        ..RpcConfig::default_with_port(rpc_port)
    };

    let node = Node::<TestBinding>::init_with_provider(
        fdi::Provider::default().with(
            JsonConfigProvider::default()
                .with::<Rpc<TestBinding>>(rpc_config)
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

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30001;
    let node = init_rpc(&temp_dir, genesis_path, port).await;

    wait_for_server_start(port).await?;

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let response = FleekApiClient::get_flk_balance(&client, eth_address, None).await?;

    assert_eq!(HpUfixed::<18>::from(1_000_u32), response);

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

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30002;
    let node = init_rpc(&temp_dir, genesis_path, port).await;

    wait_for_server_start(port).await?;

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let response = FleekApiClient::get_reputation(&client, node_public_key, None).await?;

    assert_eq!(Some(46), response);

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

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30003;
    let node = init_rpc(&temp_dir, genesis_path, port).await;
    wait_for_server_start(port).await?;

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let response = FleekApiClient::get_staked(&client, node_public_key, None).await?;

    assert_eq!(HpUfixed::<18>::from(1_000_u32), response);

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

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30004;
    let node = init_rpc(&temp_dir, genesis_path, port).await;

    wait_for_server_start(port).await?;

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let response = FleekApiClient::get_stables_balance(&client, eth_address, None).await?;

    assert_eq!(HpUfixed::<6>::from(2_00_u32), response);

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

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30005;
    let node = init_rpc(&temp_dir, genesis_path, port).await;

    wait_for_server_start(port).await?;

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let response = FleekApiClient::get_stake_locked_until(&client, node_public_key, None).await?;

    assert_eq!(365, response);

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
    let node_secret_key: NodeSecretKey = NodeSecretKey::generate();
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

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30006;
    let node = init_rpc(&temp_dir, genesis_path, port).await;

    wait_for_server_start(port).await?;

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let response = FleekApiClient::get_locked_time(&client, node_public_key, None).await?;

    assert_eq!(2, response);

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

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30007;
    let node = init_rpc(&temp_dir, genesis_path, port).await;

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

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30008;
    let node = init_rpc(&temp_dir, genesis_path, port).await;

    wait_for_server_start(port).await?;

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let response = FleekApiClient::get_bandwidth_balance(&client, eth_address, None).await?;

    assert_eq!(10_000, response);

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

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30009;
    let node = init_rpc(&temp_dir, genesis_path, port).await;

    wait_for_server_start(port).await?;

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let response = FleekApiClient::get_node_info(&client, node_public_key, None).await?;

    assert_eq!(Some(NodeInfo::from(&node_info)), response);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_staking_amount() -> Result<()> {
    let temp_dir = tempdir().unwrap();
    let genesis_path = Genesis::default()
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30010;
    let node = init_rpc(&temp_dir, genesis_path, port).await;

    wait_for_server_start(port).await?;

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let response = FleekApiClient::get_staking_amount(&client).await?;

    assert_eq!(node.query_runner().get_staking_amount(), response);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_committee_members() -> Result<()> {
    let temp_dir = tempdir().unwrap();
    let genesis_path = Genesis::default()
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30011;
    let node = init_rpc(&temp_dir, genesis_path, port).await;

    wait_for_server_start(port).await?;

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let response = FleekApiClient::get_committee_members(&client, None).await?;

    assert_eq!(node.query_runner().get_committee_members(), response);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_epoch() -> Result<()> {
    let temp_dir = tempdir().unwrap();
    let genesis_path = Genesis::default()
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30012;
    let node = init_rpc(&temp_dir, genesis_path, port).await;

    wait_for_server_start(port).await?;

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let response = FleekApiClient::get_epoch(&client).await?;

    assert_eq!(node.query_runner().get_current_epoch(), response);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_epoch_info() -> Result<()> {
    let temp_dir = tempdir().unwrap();
    let genesis_path = Genesis::default()
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30013;
    let node = init_rpc(&temp_dir, genesis_path, port).await;

    wait_for_server_start(port).await?;

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let response = FleekApiClient::get_epoch_info(&client).await?;

    assert_eq!(node.query_runner().get_epoch_info(), response);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_total_supply() -> Result<()> {
    let temp_dir = tempdir().unwrap();
    let genesis_path = Genesis::default()
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30014;
    let node = init_rpc(&temp_dir, genesis_path, port).await;

    wait_for_server_start(port).await?;

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let response = FleekApiClient::get_total_supply(&client, None).await?;

    let total_supply = match node.query_runner().get_metadata(&Metadata::TotalSupply) {
        Some(Value::HpUfixed(s)) => s,
        _ => panic!("TotalSupply is set genesis and should never be empty"),
    };

    assert_eq!(total_supply, response);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_year_start_supply() -> Result<()> {
    let temp_dir = tempdir().unwrap();
    let genesis_path = Genesis::default()
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30015;
    let node = init_rpc(&temp_dir, genesis_path, port).await;

    wait_for_server_start(port).await?;

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let response = FleekApiClient::get_year_start_supply(&client, None).await?;

    let supply_year_start = match node.query_runner().get_metadata(&Metadata::SupplyYearStart) {
        Some(Value::HpUfixed(s)) => s,
        _ => panic!("SupplyYearStart is set genesis and should never be empty"),
    };

    assert_eq!(supply_year_start, response);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_protocol_fund_address() -> Result<()> {
    let temp_dir = tempdir().unwrap();
    let genesis_path = Genesis::default()
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30016;
    let node = init_rpc(&temp_dir, genesis_path, port).await;

    wait_for_server_start(port).await?;

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let response = FleekApiClient::get_protocol_fund_address(&client).await?;

    let protocol_account = match node
        .query_runner()
        .get_metadata(&Metadata::ProtocolFundAddress)
    {
        Some(Value::AccountPublicKey(s)) => s,
        _ => panic!("AccountPublicKey is set genesis and should never be empty"),
    };

    assert_eq!(protocol_account, response);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_protocol_params() -> Result<()> {
    let temp_dir = tempdir().unwrap();
    let genesis_path = Genesis::default()
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30017;
    let node = init_rpc(&temp_dir, genesis_path, port).await;

    wait_for_server_start(port).await?;

    let params = ProtocolParams::LockTime;

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let response = FleekApiClient::get_protocol_params(&client, params.clone()).await?;

    assert_eq!(
        node.query_runner().get_protocol_param(&params).unwrap(),
        response
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
    genesis.total_served.insert(0, total_served.clone().into());

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30018;
    let node = init_rpc(&temp_dir, genesis_path, port).await;

    wait_for_server_start(port).await?;

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let response = FleekApiClient::get_total_served(&client, 0).await?;

    assert_eq!(total_served, response);

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
    genesis_node.current_epoch_served = Some(GenesisNodeServed {
        served: vec![1000],
        ..Default::default()
    });
    genesis.node_info.push(genesis_node);

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30019;
    let node = init_rpc(&temp_dir, genesis_path, port).await;

    wait_for_server_start(port).await?;

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let response = FleekApiClient::get_node_served(&client, node_public_key, None).await?;

    assert_eq!(vec![1000], response.served);

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

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30020;
    let node = init_rpc(&temp_dir, genesis_path, port).await;
    wait_for_server_start(port).await?;

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let response = FleekApiClient::is_valid_node(&client, node_public_key).await?;

    assert!(response);

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

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30021;
    let node = init_rpc(&temp_dir, genesis_path, port).await;

    wait_for_server_start(port).await?;

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let response = FleekApiClient::get_node_registry(&client, None).await?;

    assert_eq!(response.len(), committee_size + 1);
    assert!(response.contains(&NodeInfo::from(&node_info)));

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let response = FleekApiClient::get_node_registry(&client, None).await?;

    assert!(response.contains(&NodeInfo::from(&node_info)));

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_admin_seq() -> Result<()> {
    let temp_dir = tempdir().unwrap();
    let genesis_path = Genesis::default()
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30022;
    let node = init_rpc(&temp_dir, genesis_path, port).await;
    let secret = super::load_hmac_secret(Some(temp_dir.path().to_path_buf()))?;

    wait_for_server_start(port).await?;

    test_admin_client_can_call(port, &node, &secret).await?;

    node.shutdown().await;

    println!("seq test complete");

    Ok(())
}

async fn test_admin_client_can_call(port: u16, _node: &TestNode, secret: &[u8; 32]) -> Result<()> {
    let address = format!("http://127.0.0.1:{port}/admin");
    let client = RpcClient::new(&address, Some(secret)).await?;

    for _ in 0..5 {
        if let Err(e) = AdminApiClient::ping(&client).await {
            panic!("Expected OK got Error: {:?}", e);
        };
    }

    let regular_client = RpcClient::new_no_auth(&address)?;

    println!("{:?}", AdminApiClient::ping(&regular_client).await);

    assert!(AdminApiClient::ping(&regular_client).await.is_err());

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

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30023;
    let node = init_rpc(&temp_dir, genesis_path, port).await;

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

    sender.send(vec![event.clone()]);

    assert_eq!(sub.next().await.expect("An event from the sub")?, event);

    node.shutdown().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_state_root() -> Result<()> {
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

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30024;
    let node = init_rpc(&temp_dir, genesis_path, port).await;

    wait_for_server_start(port).await?;

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let root_hash = FleekApiClient::get_state_root(&client, None)
        .await?
        .to_string();

    assert_eq!(root_hash.len(), 64);
    assert!(root_hash.chars().all(|c| c.is_ascii_hexdigit()));

    node.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rpc_get_state_proof() -> Result<()> {
    let temp_dir = tempdir()?;

    // Create keys
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();
    let owner_eth_address: EthAddress = owner_public_key.into();

    // Init application service
    let mut genesis = Genesis::default();
    genesis.account.push(GenesisAccount {
        public_key: owner_public_key.into(),
        flk_balance: 1000u64.into(),
        stables_balance: 0,
        bandwidth_balance: 0,
    });

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let port = 30025;
    let node = init_rpc(&temp_dir, genesis_path, port).await;

    wait_for_server_start(port).await?;

    let client = RpcClient::new_no_auth(&format!("http://127.0.0.1:{port}/rpc/v0"))?;
    let state_key = StateProofKey::Accounts(owner_eth_address);
    let (value, proof) =
        FleekApiClient::get_state_proof(&client, StateProofKey::Accounts(owner_eth_address), None)
            .await?;

    assert!(value.is_some());
    let value = value.unwrap();
    assert_eq!(
        value.clone(),
        StateProofValue::Accounts(AccountInfo {
            flk_balance: 1000u64.into(),
            stables_balance: HpUfixed::zero(),
            bandwidth_balance: 0,
            nonce: 0,
        })
    );

    // Verify proof.
    let root_hash = FleekApiClient::get_state_root(&client, None).await?;
    proof
        .verify_membership::<_, _, ApplicationMerklizeProvider>(
            state_key.table(),
            owner_eth_address,
            AccountInfo {
                flk_balance: 1000u64.into(),
                stables_balance: HpUfixed::zero(),
                bandwidth_balance: 0,
                nonce: 0,
            },
            root_hash,
        )
        .unwrap();

    node.shutdown().await;
    Ok(())
}
