use anyhow::{anyhow, Result};
use reqwest::{Client, Response};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct RpcSuccessResponse<T> {
    pub jsonrpc: String,
    pub id: usize,
    pub result: T,
}

pub async fn make_request(address: String, req: String) -> Result<Response> {
    let client = Client::new();
    Ok(client
        .post(address)
        .header("Content-Type", "application/json")
        .body(req)
        .send()
        .await?)
}

pub async fn rpc_request<T: DeserializeOwned>(
    client: &reqwest::Client,
    address: String,
    req: String,
) -> Result<RpcSuccessResponse<T>> {
    let res = client
        .post(address)
        .header("Content-Type", "application/json")
        .body(req)
        .send()
        .await?;

    if res.status().is_success() {
        let value: serde_json::Value = res.json().await?;
        if value.get("result").is_some() {
            let value: RpcSuccessResponse<T> = serde_json::from_value(value)?;
            Ok(value)
        } else {
            Err(anyhow!("Failed to parse response"))
        }
    } else {
        Err(anyhow!("Request failed with status: {}", res.status()))
    }
}
