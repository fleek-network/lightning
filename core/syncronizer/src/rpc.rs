use std::net::IpAddr;

use anyhow::{anyhow, Result};
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
