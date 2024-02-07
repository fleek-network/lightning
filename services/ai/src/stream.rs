use bytes::{Buf, BytesMut};
use fn_sdk::connection::Connection;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::Request;

#[derive(Debug, Serialize, Deserialize)]
pub struct Data {
    origin: Origin,
    uri: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[repr(u8)]
pub enum Origin {
    Blake3,
    Ipfs,
    Http,
    Unknown,
}

#[allow(dead_code)]
pub struct ServiceStream {
    connection: Connection,
    buffer: BytesMut,
}

#[allow(dead_code)]
impl ServiceStream {
    pub fn new(connection: Connection) -> Self {
        Self {
            connection,
            buffer: BytesMut::new(),
        }
    }

    // Note: this is not cancel safe.
    pub async fn recv(&mut self) -> Option<Request> {
        while self.buffer.len() < 5 {
            if self
                .connection
                .stream
                .read_buf(&mut self.buffer)
                .await
                .ok()?
                == 0
            {
                return None;
            }
        }

        let len = self.buffer.get_u32() as usize;

        if len == 0 {
            return None;
        }

        self.buffer.reserve(len + 4);

        while self.buffer.len() < len {
            if self
                .connection
                .stream
                .read_buf(&mut self.buffer)
                .await
                .ok()?
                == 0
            {
                return None;
            }
        }

        serde_json::from_slice(&self.buffer.split_to(len)).ok()
    }

    pub async fn send(&mut self, data: &[u8]) -> anyhow::Result<()> {
        self.connection.start_write(data.len()).await?;
        self.connection.write_all(data).await?;
        Ok(())
    }
}
