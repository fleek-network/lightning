use std::future::Future;
use std::net::IpAddr;

use anyhow::{anyhow, Result};
use fleek_crypto::NodePublicKey;
use lightning_interfaces::types::{EpochInfo, NodeIndex, NodeInfo};
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
    nodes: &Vec<(NodeIndex, NodeInfo)>,
    rpc_client: &reqwest::Client,
) -> Result<T> {
    for (_, node) in nodes {
        if let Ok(res) =
            rpc_request::<T>(rpc_client, node.domain, node.ports.rpc, req.clone()).await
        {
            return Ok(res.result);
        }
    }
    Err(anyhow!("Unable to get a responce from nodes"))
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
    ask_nodes(rpc_epoch_info().to_string(), &nodes, &rpc_client).await
}

/// Returns the node info for our node, if it's already on the state.
pub async fn get_node_info(
    node_public_key: NodePublicKey,
    nodes: Vec<(NodeIndex, NodeInfo)>,
    rpc_client: reqwest::Client,
) -> Result<Option<NodeInfo>> {
    ask_nodes(
        rpc_node_info(&node_public_key).to_string(),
        &nodes,
        &rpc_client,
    )
    .await
}

/// Returns the node info for our node, if it's already on the state.
pub async fn check_is_valid_node(
    node_public_key: NodePublicKey,
    nodes: Vec<(NodeIndex, NodeInfo)>,
    rpc_client: reqwest::Client,
) -> Result<bool> {
    ask_nodes(
        rpc_is_valid_node(&node_public_key).to_string(),
        &nodes,
        &rpc_client,
    )
    .await
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
        "method":"flk_get_node_info",
        "params":{"public_key": public_key},
        "id":1,
    })
}

pub fn rpc_is_valid_node(public_key: &NodePublicKey) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method":"flk_is_valid_node",
        "params":{"public_key": public_key},
        "id":1,
    })
}
