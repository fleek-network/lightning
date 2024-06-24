use anyhow::Result;
use reqwest::{Client, Response};
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

pub fn get_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Can get time since unix epoch")
        .as_secs()
}

/// Warning! assumes the address is of the form http://ip:port/admin
pub async fn get_admin_nonce(client: &reqwest::Client, address: String) -> Result<u32> {
    Ok(client
        .get(format!("{}/nonce", address))
        .send()
        .await?
        .text()
        .await?
        .parse()?)
}
