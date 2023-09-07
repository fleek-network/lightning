use std::fs;
use std::net::IpAddr;
use std::time::Duration;

use anyhow::{anyhow, Result};
use atomo_rocks::{Cache as RocksCache, Env as RocksEnv, Options};
use fleek_crypto::{ConsensusSecretKey, NodePublicKey, NodeSecretKey, SecretKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_application::config::Config as AppConfig;
use lightning_application::env::Env;
use lightning_application::genesis::{Genesis, GenesisNode};
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore::config::Config as BlockstoreConfig;
use lightning_blockstore_server::config::Config as BlockServerConfig;
use lightning_blockstore_server::BlockStoreServer;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::{
    partial,
    BlockStoreInterface,
    BlockStoreServerInterface,
    SyncQueryRunnerInterface,
};
use lightning_rpc::handlers::RPC_VERSION;
use lightning_signer::Config as SignerConfig;
use lightning_types::{Epoch, NodeInfo};
use log::{error, info, warn};
use serde::de::DeserializeOwned;

partial!(PartialBinding {
    BlockStoreInterface = Blockstore<Self>;
    BlockStoreServerInterface = BlockStoreServer<Self>;
});

pub async fn sync(
    signer_config: SignerConfig,
    app_config: AppConfig,
    blockstore_config: BlockstoreConfig,
    block_server_config: BlockServerConfig,
) {
    let genesis = Genesis::load().expect("Failed to load genesis.");
    let on_committee = am_i_on_the_committee(&genesis, &signer_config);
    loop {
        let is_whitelisted = am_i_whitelisted(&genesis, &signer_config).await;
        if is_whitelisted || on_committee {
            break;
        } else {
            error!(
                "This node is not whitelisted for testnet. Please checkout discord.gg/fleekxyz to learn how to get whitelisted"
            );
            error!("checking whitelist again in 5 minutes");

            tokio::time::sleep(Duration::from_secs(300)).await;
        }
    }

    let mut env = Env::new(&app_config, None).expect("Failed to initialize environment.");
    if !env.genesis(&app_config) {
        info!("State already exists. Not loading genesis.");
    }

    let nodes_to_sync = find_nodes_for_syncing(genesis).await;
    if nodes_to_sync.is_empty() && !on_committee {
        panic!("This testnet phase has ended. Stay tuned for the next phase on the Fleek Discord.");
    }

    let my_epoch = env.query_runner().get_epoch();
    let mut max_epoch = my_epoch;
    nodes_to_sync
        .iter()
        .for_each(|(epoch, _)| max_epoch = max_epoch.max(*epoch));
    if my_epoch == max_epoch {
        info!("No genesis committee members with higher epoch found. Not syncing state.");
        return;
    }
    let nodes_to_sync: Vec<GenesisNode> = nodes_to_sync
        .into_iter()
        .filter_map(
            |(epoch, node)| {
                if epoch == max_epoch { Some(node) } else { None }
            },
        )
        .collect();
    if nodes_to_sync.is_empty() {
        info!("No genesis committee members with higher epoch found. Not syncing state.");
        return;
    }

    std::mem::drop(env);
    let blockstore = Blockstore::<PartialBinding>::init(blockstore_config)
        .expect("Failed to initialize blockstore");
    let block_server =
        BlockStoreServer::<PartialBinding>::init(block_server_config, blockstore.clone())
            .expect("Failed to initialize blockstore server");
    attempt_to_sync_state::<PartialBinding>(nodes_to_sync, &app_config, blockstore, block_server)
        .await
        .expect("Failed to synchronize state. Abort mission.");
}

async fn find_nodes_for_syncing(genesis: Genesis) -> Vec<(Epoch, GenesisNode)> {
    let client = reqwest::Client::new();
    let mut nodes_to_sync = Vec::new();
    for node in &genesis.node_info {
        if !node.genesis_committee {
            continue;
        }
        if let Ok(res) = rpc_request::<Epoch>(
            &client,
            node.primary_domain,
            node.ports.rpc,
            rpc_epoch().to_string(),
        )
        .await
        {
            nodes_to_sync.push((res.result, node.clone()));
        }
    }
    nodes_to_sync
}

fn load_keys(config: &SignerConfig) -> (NodeSecretKey, ConsensusSecretKey) {
    let node_secret_key = if config.node_key_path.exists() {
        let node_secret_key =
            fs::read_to_string(&config.node_key_path).expect("Failed to read node pem file");
        NodeSecretKey::decode_pem(&node_secret_key).expect("Failed to decode node pem file")
    } else {
        panic!("Node key does not exist. Use CLI to generate keys.");
    };

    let consensus_secret_key = if config.consensus_key_path.exists() {
        let consensus_secret_key = fs::read_to_string(&config.consensus_key_path)
            .expect("Failed to read consensus pem file");
        ConsensusSecretKey::decode_pem(&consensus_secret_key)
            .expect("Failed to decode consensus pem file")
    } else {
        panic!("Consensus key does not exist. Use CLI to generate keys.");
    };
    (node_secret_key, consensus_secret_key)
}

async fn attempt_to_sync_state<C: Collection>(
    nodes_to_sync: Vec<GenesisNode>,
    config: &AppConfig,
    blockstore: C::BlockStoreInterface,
    blockstore_server: C::BlockStoreServerInterface,
) -> Result<()> {
    let mut db_options = if let Some(db_options) = config.db_options.as_ref() {
        let (options, _) = Options::load_latest(
            db_options,
            RocksEnv::new().expect("Failed to create rocks db env."),
            false,
            RocksCache::new_lru_cache(100),
        )
        .expect("Failed to create rocks db options.");
        options
    } else {
        Options::default()
    };
    db_options.create_if_missing(true);
    db_options.create_missing_column_families(true);

    let client = reqwest::Client::new();

    for node in nodes_to_sync {
        if let Ok(res) = rpc_request::<[u8; 32]>(
            &client,
            node.primary_domain,
            node.ports.rpc,
            rpc_last_epoch_hash().to_string(),
        )
        .await
        {
            let hash = res.result;
            let address = format!("{}:{}", node.primary_domain, node.ports.blockstore)
                .parse()
                .unwrap();
            if let Err(e) = blockstore_server.request_download(hash, address).await {
                warn!(
                    "Failed to download checkpoint with hash {hash:?} from node {}: {e:?}",
                    node.primary_public_key
                );
                continue;
            }
            if let Some(checkpoint) = blockstore.read_all_to_vec(&hash).await {
                // Attempt to build db from checkpoint.
                match Env::new(config, Some((hash, checkpoint))) {
                    Ok(mut env) => {
                        info!("Successfully built database from checkpoint with hash {hash:?}.");

                        // Update the last epoch has on state
                        env.update_last_epoch_hash(hash);

                        return Ok(());
                    },
                    Err(e) => {
                        warn!(
                            "Failed to built database from checkpoint with hash {hash:?} that we received from node {:?}: {e:?}",
                            node.primary_public_key
                        );
                    },
                }
            }
        }
    }
    Err(anyhow!("Failed to sync state."))
}

pub fn am_i_on_the_committee(genesis: &Genesis, signer_config: &SignerConfig) -> bool {
    let (node_secret_key, _) = load_keys(signer_config);
    let node_pub_key = node_secret_key.to_pk();

    // Are we on the genesis committee?
    for member in &genesis.node_info {
        if member.genesis_committee && member.primary_public_key == node_pub_key {
            return true;
        }
    }
    false
}

pub async fn am_i_whitelisted(genesis: &Genesis, signer_config: &SignerConfig) -> bool {
    let (node_secret_key, _) = load_keys(signer_config);
    let node_pub_key = node_secret_key.to_pk();

    // Are we whitelisted?
    let client = reqwest::Client::new();
    for member in &genesis.node_info {
        if !member.genesis_committee {
            continue;
        }
        if let Ok(res) = rpc_request::<Option<NodeInfo>>(
            &client,
            member.primary_domain,
            member.ports.rpc,
            rpc_node_info(&node_pub_key, RPC_VERSION).to_string(),
        )
        .await
        {
            if let Some(node_info) = res.result {
                if node_info.public_key == node_pub_key
                    && node_info.stake.staked > HpUfixed::<18>::zero()
                {
                    return true;
                }
            }
        }
    }
    false
}

async fn rpc_request<T: DeserializeOwned>(
    client: &reqwest::Client,
    ip: IpAddr,
    port: u16,
    req: String,
) -> Result<RpcResponse<T>> {
    let res = client
        .post(format!("http://{ip}:{port}/rpc/v0"))
        .header("Content-Type", "application/json")
        .body(req)
        .send()
        .await?;
    if res.status().is_success() {
        let value: serde_json::Value = res.json().await?;
        if value.get("result").is_some() {
            let value: RpcResponse<T> = serde_json::from_value(value)?;
            Ok(value)
        } else {
            Err(anyhow!("Failed to parse response"))
        }
    } else {
        Err(anyhow!("Request failed with status: {}", res.status()))
    }
}

fn rpc_epoch() -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method":"flk_get_epoch",
        "params":[],
        "id":1,
    })
}

fn rpc_last_epoch_hash() -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method":"flk_get_last_epoch_hash",
        "params":[],
        "id":1,
    })
}

fn rpc_node_info(node_public_key: &NodePublicKey, version: u8) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method":"flk_get_node_info",
        "params":{"public_key": node_public_key, "version": version },
        "id":1,
    })
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct RpcResponse<T> {
    jsonrpc: String,
    id: usize,
    result: T,
}
