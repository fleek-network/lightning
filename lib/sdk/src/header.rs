use std::collections::HashMap;

use anyhow::{anyhow, Result};
use fleek_crypto::ClientPublicKey;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;
use url::Url;

use crate::io_util::read_length_delimited;

/// The header of this connection.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConnectionHeader {
    pub pk: Option<ClientPublicKey>,
    pub transport_detail: TransportDetail,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

///  Response type used by a service to override the handshake http response fields when the
/// transport is HTTP
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HttpResponse {
    pub headers: Option<Vec<(String, String)>>,
    pub status: Option<u16>,
    pub body: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct HttpOverrides {
    pub headers: Option<Vec<(String, String)>>,
    pub status: Option<u16>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TransportDetail {
    HttpRequest {
        method: HttpMethod,
        uri: Url,
        header: HashMap<String, String>,
    },
    Other,
}

pub async fn read_header(stream: &mut UnixStream) -> Option<ConnectionHeader> {
    let buffer = read_length_delimited(stream).await?;
    serde_cbor::from_slice(&buffer).ok()
}

pub async fn write_header(
    header: &ConnectionHeader,
    stream: &mut UnixStream,
) -> Result<(), WriteHeaderError> {
    let mut buffer = Vec::with_capacity(256);
    buffer.extend_from_slice(&[0, 0, 0, 0]);
    serde_cbor::to_writer(&mut buffer, header)?;
    let size = (buffer.len() - 4) as u32;
    buffer[0..4].copy_from_slice(&size.to_be_bytes());
    stream.write_all(&buffer).await?;
    Ok(())
}

#[derive(Debug, Error)]
#[error(transparent)]
pub enum WriteHeaderError {
    Serialization(#[from] serde_cbor::Error),
    Write(#[from] std::io::Error),
}

impl HttpResponse {
    pub fn try_from_json(value: &serde_json::Value) -> Result<Self> {
        if value["status"].is_null() {
            return Err(anyhow!("Failed to parse status"));
        }
        let Some(status) = value["status"].as_str() else {
            return Err(anyhow!("Failed to parse status"));
        };
        let status = status.parse::<u16>()?;
        println!("status: {:?}", status);

        if value["body"].is_null() {
            return Err(anyhow!("Failed to parse body"));
        }
        let body = value["body"].to_string();
        println!("body: {}", body);

        if value["headers"].is_null() {
            return Err(anyhow!("Failed to parse body"));
        }
        println!("array: {}", value["headers"].is_object());

        todo!()
    }
}

#[cfg(test)]
mod tests {
    use crate::header::HttpResponse;

    #[test]
    fn test_json() {
        let json_str = std::fs::read_to_string("/home/matthias/Desktop/ssr.txt").unwrap();
        let value: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        let res = HttpResponse::try_from_json(&value).unwrap();

        println!("{value}");
    }
}
