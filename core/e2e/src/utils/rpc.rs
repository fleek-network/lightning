use anyhow::Result;
use reqwest::{Client, Response};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub async fn parse_response<T: DeserializeOwned>(response: Response) -> Result<T> {
    if !response.status().is_success() {
        panic!("Request failed with status: {}", response.status());
    }
    let value: Value = response.json().await?;
    if value.get("result").is_some() {
        let success_res: RpcSuccessResponse<T> = serde_json::from_value(value)?;
        Ok(success_res.result)
    } else {
        panic!("Rpc Error: {value}")
    }
}

pub async fn rpc_request(address: String, request: String) -> Result<Response> {
    let client = Client::new();
    Ok(client
        .post(address)
        .header("Content-Type", "application/json")
        .body(request)
        .send()
        .await?)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RpcSuccessResponse<T> {
    jsonrpc: String,
    id: usize,
    result: T,
}
