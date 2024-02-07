use std::collections::HashMap;

use fleek_crypto::ClientPublicKey;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use url::Url;

/// The header of this connection.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConnectionHeader {
    pub pk: Option<ClientPublicKey>,
    pub transport_detail: TransportDetail,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub enum HttpMethod {
    Post,
    Get,
    Put,
    Delete,
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
    let mut size = [0; 4];
    // really unnecessary.
    let mut i = 0;
    while i < 4 {
        match stream.read(&mut size[i..]).await {
            Ok(0) | Err(_) => return None,
            Ok(n) => {
                i += n;
            },
        }
    }
    let size = u32::from_be_bytes(size) as usize;
    // now let's read `size` bytes.
    let mut buffer = Vec::with_capacity(size);
    while buffer.len() < size {
        match stream.read_buf(&mut buffer).await {
            Ok(0) | Err(_) => return None,
            Ok(_) => {},
        }
    }
    debug_assert_eq!(buffer.len(), size);
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
