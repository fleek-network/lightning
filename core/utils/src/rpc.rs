use anyhow::{anyhow, Result};
use reqwest::header::HeaderMap;
use reqwest::{Client, Response};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct RpcSuccessResponse<T> {
    pub jsonrpc: String,
    pub id: usize,
    pub result: T,
}

pub struct RpcAdminHeaders {
    pub hmac: String,
    pub nonce: u32,
    pub timestamp: u64,
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

pub fn get_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Can get time since unix epoch")
        .as_secs()
}

pub async fn get_admin_nonce(client: &reqwest::Client, address: String) -> Result<u32> {
    Ok(client
        .get(format!("{}/admin/nonce", address))
        .send()
        .await?
        .text()
        .await?
        .parse()?)
}

pub async fn rpc_request<T: DeserializeOwned>(
    client: &reqwest::Client,
    address: String,
    req: String,
    maybe_hmac: Option<RpcAdminHeaders>,
) -> Result<RpcSuccessResponse<T>> {
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "application/json".parse().unwrap());

    if let Some(hmac) = maybe_hmac {
        headers.insert("X-Lightning-HMAC", hmac.hmac.parse().unwrap());
        headers.insert(
            "X-Lightning-Timestamp",
            hmac.timestamp.to_string().parse().unwrap(),
        );
        headers.insert("X-Lightning-Nonce", hmac.nonce.to_string().parse().unwrap());
    }

    let res = client
        .post(address)
        .headers(headers)
        .body(req)
        .send()
        .await?;

    if res.status().is_success() {
        let value: serde_json::Value = res.json().await?;
        println!("{:?}", value);

        if value.get("result").is_some() {
            let value: RpcSuccessResponse<T> = serde_json::from_value(value)?;
            Ok(value)
        } else {
            Err(anyhow!("Failed to parse response"))
        }
    } else {
        Err(anyhow!(
            "Request failed with status: {}, err {}",
            res.status(),
            res.text().await?
        ))
    }
}
