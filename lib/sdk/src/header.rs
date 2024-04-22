use std::collections::HashMap;

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
    pub body: Vec<u8>,
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
