use anyhow::Result;
use arrayref::array_ref;
use bytes::BytesMut;
use fn_sdk::api::Origin as ApiOrigin;
use fn_sdk::connection::Connection;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Request to execute some javascript from an origin
#[derive(Serialize, Deserialize)]
pub struct Request {
    /// Origin to use
    pub origin: Origin,
    /// URI For the origin
    /// - for blake3 should be hex encoded bytes
    /// - for ipfs should be cid string
    pub uri: String,
    /// Parameter to pass to the script's main function
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
#[repr(u8)]
pub enum Origin {
    Blake3,
    Ipfs,
    Unknown,
}

impl From<Origin> for ApiOrigin {
    #[inline(always)]
    fn from(val: Origin) -> Self {
        match val {
            Origin::Ipfs => ApiOrigin::IPFS,
            _ => unreachable!(),
        }
    }
}

pub struct ServiceStream {
    pub conn: Connection,
    buffer: BytesMut,
}

impl ServiceStream {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn,
            buffer: BytesMut::with_capacity(1),
        }
    }

    pub async fn read_request(&mut self) -> Option<Request> {
        // Read the payload length delimiter
        while self.buffer.len() < 5 {
            if self.conn.stream.read_buf(&mut self.buffer).await.ok()? == 0 {
                // Socket was closed
                return None;
            }
        }

        // Parse and allocate for the length
        let bytes = self.buffer.split_to(4);
        let len = u32::from_be_bytes(*array_ref!(bytes, 0, 4)) as usize;
        if len == 0 || len > 1025 {
            // If the client specified it's going to send 0 bytes, this is an error
            return None;
        }

        // We reserve an additional 4 bytes for the next request
        self.buffer.reserve(len + 4);

        // Read the request
        while self.buffer.len() < len {
            if self.conn.stream.read_buf(&mut self.buffer).await.ok()? == 0 {
                // Socket was closed
                return None;
            }
        }

        let bytes = self.buffer.split_to(len);
        serde_json::from_slice(&bytes).ok()
    }

    pub async fn send_payload(&mut self, bytes: &[u8]) -> Result<()> {
        self.conn.start_write(bytes.len()).await?;
        self.conn.write_all(bytes).await?;
        Ok(())
    }
}
