use std::future::Future;
use std::net::IpAddr;

use anyhow::{anyhow, Result};
use fleek_crypto::NodePublicKey;
use lightning_interfaces::types::{Epoch, EpochInfo, NodeIndex, NodeInfo};
use serde::de::DeserializeOwned;
use tokio::runtime::Handle;

pub async fn rpc_request<T: DeserializeOwned>(
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

pub async fn ask_nodes<T: DeserializeOwned>(
    req: String,
    nodes: &[(NodeIndex, NodeInfo)],
    rpc_client: &reqwest::Client,
) -> Result<Vec<T>> {
    let mut futs = Vec::new();
    for (_, node) in nodes {
        let req_clone = req.clone();
        let fut = async move {
            rpc_request::<T>(rpc_client, node.domain, node.ports.rpc, req_clone)
                .await
                .ok()
        };
        futs.push(fut);
    }

    let results: Vec<T> = futures::future::join_all(futs)
        .await
        .into_iter()
        .flatten()
        .map(|x| x.result)
        .collect();

    if results.is_empty() {
        Err(anyhow!("Unable to get a response from nodes"))
    } else {
        Ok(results)
    }
}

/// Runs the given future to completion on the current tokio runtime.
/// This call is intentionally blocking.
pub fn sync_call<F>(fut: F) -> F::Output
where
    F: Future + Send + 'static,
    F::Output: Send,
{
    let handle = Handle::current();
    std::thread::spawn(move || handle.block_on(fut))
        .join()
        .unwrap()
}

/// Returns the epoch info from the epoch the bootstrap nodes are on
pub async fn get_epoch_info(
    nodes: Vec<(NodeIndex, NodeInfo)>,
    rpc_client: reqwest::Client,
) -> Result<EpochInfo> {
    let mut epochs: Vec<EpochInfo> =
        ask_nodes(rpc_epoch_info().to_string(), &nodes, &rpc_client).await?;
    if epochs.is_empty() {
        return Err(anyhow!("Failed to get epoch info from bootstrap nodes"));
    }
    epochs.sort_by(|a, b| a.epoch.partial_cmp(&b.epoch).unwrap());
    Ok(epochs.pop().unwrap())
}

/// Returns the node info for our node, if it's already on the state.
pub async fn get_node_info(
    node_public_key: NodePublicKey,
    nodes: Vec<(NodeIndex, NodeInfo)>,
    rpc_client: reqwest::Client,
) -> Result<Option<NodeInfo>> {
    let mut node_info: Vec<(Option<NodeInfo>, Epoch)> = ask_nodes(
        rpc_node_info(&node_public_key).to_string(),
        &nodes,
        &rpc_client,
    )
    .await?;

    if node_info.is_empty() {
        return Err(anyhow!("Failed to get node info from bootstrap nodes"));
    }
    node_info.sort_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap());
    Ok(node_info.pop().unwrap().0)
}

/// Returns the node info for our node, if it's already on the state.
pub async fn check_is_valid_node(
    node_public_key: NodePublicKey,
    nodes: Vec<(NodeIndex, NodeInfo)>,
    rpc_client: reqwest::Client,
) -> Result<bool> {
    let mut is_valid: Vec<(bool, Epoch)> = ask_nodes(
        rpc_is_valid_node(&node_public_key).to_string(),
        &nodes,
        &rpc_client,
    )
    .await?;

    if is_valid.is_empty() {
        return Err(anyhow!("Failed to get node validity from bootstrap nodes"));
    }
    is_valid.sort_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap());
    Ok(is_valid.pop().unwrap().0)
}

/// Returns the hash of the last epoch ckpt, and the current epoch.
pub async fn last_epoch_hash(
    nodes: &[(NodeIndex, NodeInfo)],
    rpc_client: &reqwest::Client,
) -> Result<[u8; 32]> {
    let mut hash: Vec<([u8; 32], Epoch)> =
        ask_nodes(rpc_last_epoch_hash().to_string(), nodes, rpc_client).await?;

    if hash.is_empty() {
        return Err(anyhow!(
            "Failed to get last epoch hash from bootstrap nodes"
        ));
    }
    hash.sort_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap());
    Ok(hash.pop().unwrap().0)
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct RpcResponse<T> {
    pub jsonrpc: String,
    pub id: usize,
    pub result: T,
}

// todo(dalton): Lazy static?
pub fn rpc_last_epoch_hash() -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method":"flk_get_last_epoch_hash",
        "params":[],
        "id":1,
    })
}

pub fn rpc_epoch() -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method":"flk_get_epoch",
        "params":[],
        "id":1,
    })
}

pub fn rpc_epoch_info() -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method":"flk_get_epoch_info",
        "params":[],
        "id":1,
    })
}

pub fn rpc_node_info(public_key: &NodePublicKey) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method":"flk_get_node_info_epoch",
        "params":{"public_key": public_key},
        "id":1,
    })
}

pub fn rpc_is_valid_node(public_key: &NodePublicKey) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method":"flk_is_valid_node_epoch",
        "params":{"public_key": public_key},
        "id":1,
    })
}
