use anyhow::{anyhow, Result};
use fleek_crypto::{ClientPublicKey, ClientSignature};
use lightning_interfaces::{types::ServiceId, CompressionAlgoSet};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::connection::{
    consts::{HANDSHAKE_RES_TAG, HANDSHAKE_RES_UNLOCK_TAG},
    HandshakeConnection, HandshakeFrame,
};

pub struct HandshakeClient<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> {
    conn: HandshakeConnection<R, W>,
    pubkey: ClientPublicKey,
    compression_set: CompressionAlgoSet,
}

impl<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> HandshakeClient<R, W> {
    pub fn new(
        reader: R,
        writer: W,
        pubkey: ClientPublicKey,
        compression_set: CompressionAlgoSet,
    ) -> Self {
        let conn = HandshakeConnection::new(reader, writer);
        Self {
            conn,
            pubkey,
            compression_set,
        }
    }

    /// Handshake with a node
    pub async fn handshake(&mut self) -> Result<()> {
        // Send request
        self.conn
            .write_frame(HandshakeFrame::HandshakeRequest {
                version: 0,
                supported_compression_set: self.compression_set,
                pubkey: self.pubkey,
                resume_lane: None,
            })
            .await?;

        // Await response
        match self.conn.read_frame(Some(HANDSHAKE_RES_TAG)).await? {
            Some(HandshakeFrame::HandshakeResponse { .. }) => {
                // TODO: Verification?
            },
            Some(_) => unreachable!(),
            None => return Err(anyhow!("connection disconnected")),
        }

        Ok(())
    }

    /// Handshake with a node, specifying a lane to unlock and a signature to send.
    pub async fn handshake_unlock(&mut self, lane: u8, _signature: ClientSignature) -> Result<()> {
        // Send request
        self.conn
            .write_frame(HandshakeFrame::HandshakeRequest {
                version: 0,
                supported_compression_set: self.compression_set,
                pubkey: self.pubkey,
                resume_lane: Some(lane),
            })
            .await?;

        // Recieve response
        match self.conn.read_frame(Some(HANDSHAKE_RES_UNLOCK_TAG)).await? {
            Some(HandshakeFrame::HandshakeResponseUnlock { .. }) => {
                // TODO: Verification?
            },
            Some(_) => unreachable!(),
            None => return Err(anyhow!("connection disconnected")),
        }

        // Send delivery acknowledgement
        self.conn
            .write_frame(HandshakeFrame::DeliveryAcknowledgement {
                // TODO: Figure out size in fleek_crypto
                signature: ClientSignature,
            })
            .await?;

        Ok(())
    }

    /// Send a request to initialize a service. Returns the underlying reader and writer to
    /// pass to the service's client next.
    pub async fn request(mut self, service_id: ServiceId) -> Result<(R, W)> {
        self.conn
            .write_frame(HandshakeFrame::ServiceRequest { service_id })
            .await?;
        Ok(self.conn.finish())
    }
}
