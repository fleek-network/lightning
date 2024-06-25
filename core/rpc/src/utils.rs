use anyhow::anyhow;
pub use lightning_utils::rpc::make_request;
use lightning_utils::rpc::{get_admin_nonce, get_timestamp, RpcSuccessResponse};
use reqwest::header::HeaderMap;
use serde::de::DeserializeOwned;

use crate::create_hmac;

/// todo(n):
///
/// jsonrpsee creats typed clients for each rpc method
/// we can probaly expose those here (or somewhere) and remove anyone from using this
/// we would need to override the admin methods to use the hmac
pub async fn rpc_request<T: DeserializeOwned>(
    client: &reqwest::Client,
    address: String,
    req: String,
    hmac: Option<&[u8; 32]>,
) -> anyhow::Result<RpcSuccessResponse<T>> {
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "application/json".parse().unwrap());

    if let Some(secret) = hmac {
        let timestamp = get_timestamp();
        let nonce = get_admin_nonce(client, address.clone()).await?;
        let hmac = create_hmac(secret, req.as_bytes(), timestamp, nonce);

        headers.insert("X-Lightning-HMAC", hmac.parse().unwrap());
        headers.insert(
            "X-Lightning-Timestamp",
            timestamp.to_string().parse().unwrap(),
        );
        headers.insert("X-Lightning-Nonce", nonce.to_string().parse().unwrap());
    }

    let res = client
        .post(address)
        .headers(headers)
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
        Err(anyhow!(
            "Request failed with status: {}, err {}",
            res.status(),
            res.text().await?
        ))
    }
}
