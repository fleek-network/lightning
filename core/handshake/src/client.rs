use anyhow::{anyhow, Result};
use draco_interfaces::types::ServiceId;
use fleek_crypto::{ClientPublicKey, ClientSignature};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::connection::{
    consts::{HANDSHAKE_RES_TAG, HANDSHAKE_RES_UNLOCK_TAG},
    HandshakeConnection, HandshakeFrame,
};

pub struct HandshakeClient<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> {
    conn: HandshakeConnection<R, W>,
    pubkey: ClientPublicKey,
}

impl<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> HandshakeClient<R, W> {
    pub fn new(reader: R, writer: W, pubkey: ClientPublicKey) -> Self {
        let conn = HandshakeConnection::new(reader, writer);
        Self { conn, pubkey }
    }

    /// Handshake with a node
    pub async fn handshake(&mut self) -> Result<()> {
        // Send request
        self.conn
            .write_frame(HandshakeFrame::HandshakeRequest {
                version: 0,
                supported_compression_bitmap: 0,
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

    pub async fn handshake_unlock(&mut self, lane: u8, _signature: ClientSignature) -> Result<()> {
        // Send request
        self.conn
            .write_frame(HandshakeFrame::HandshakeRequest {
                version: 0,
                supported_compression_bitmap: 0,
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
