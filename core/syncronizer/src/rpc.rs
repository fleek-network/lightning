use std::net::IpAddr;

use anyhow::{anyhow, Context, Result};
use fleek_crypto::{NodePublicKey, PublicKey};
use serde::de::DeserializeOwned;

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
        } else if value.get("error").is_some() {
            let code = value
                .get("error")
                .unwrap()
                .get("code")
                .context("Failed to parse response")?;
            match serde_json::from_value::<u8>(code.clone()) {
                Ok(69) => Err(anyhow!("Version mismatch")).context(RequestError::VersionMismatch),
                Ok(123) => Err(anyhow!("Node is not staked")).context(RequestError::NotStaked),
                _ => Err(anyhow!("Failed to parse response")),
            }
        } else {
            Err(anyhow!("Failed to parse response"))
        }
    } else {
        Err(anyhow!("Request failed with status: {}", res.status()))
    }
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

pub fn rpc_epoch_testnet(node_public_key: NodePublicKey) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method":"flk_get_epoch_testnet",
        "params": {"public_key": node_public_key.to_base58(), "version": 1},
        "id":1,
    })
}

#[derive(Debug)]
pub enum RequestError {
    NotStaked,
    VersionMismatch,
}

impl std::fmt::Display for RequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestError::NotStaked => write!(f, "Node is not staked."),
            RequestError::VersionMismatch => {
                write!(f, "Version mismatch. Please update your binary.")
            },
        }
    }
}
