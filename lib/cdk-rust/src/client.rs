use anyhow::Result;
use bytes::Bytes;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot;

use crate::mode::Mode;

// Client for primary connections.
#[derive(Clone)]
pub struct Client {
    sender: Sender<InternalRequest>,
}

impl Client {
    /// Send a request to node.
    pub async fn request(&self, node: [u8; 32], request: Request) -> Result<Response> {
        let (tx, rx) = oneshot::channel();
        let internal = InternalRequest {
            node,
            payload: request,
            respond: tx,
        };
        self.sender
            .send(internal)
            .await
            .map_err(|_| anyhow::anyhow!("internal error: failed to send request"))?;
        rx.await
            .map_err(|_| anyhow::anyhow!("internal error: failed to receive a response"))
    }

    /// Similar as [`Builder::request`] but ignores the response.
    pub fn send(&self, node: [u8; 32], request: Request) {
        let us = self.clone();
        tokio::spawn(async move { if let Err(e) = us.request(node, request).await {} });
    }
}

pub struct InternalRequest {
    node: [u8; 32],
    payload: Request,
    respond: oneshot::Sender<Response>,
}

pub enum Request {
    ServicePayload { bytes: Bytes },
    AccessToken { ttl: u64 },
    ExtendAccessToken { ttl: u64 },
}

pub enum Response {
    ServicePayload {
        bytes: Bytes,
    },
    AccessToken {
        ttl: u64,
        access_token: Box<[u8; 48]>,
    },
    ExtendAccessTokenSuccess,
}

/// Pipe for secondary connections.
pub struct Pipe {
    sender: Sender<()>,
}
