use std::time::Duration;

use anyhow::{bail, Result};
use bytes::Bytes;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

use crate::context::Context;
use crate::schema::{RequestFrame, ResponseFrame, TerminationReason};

pub type Response = ResponseFrame;

pub trait Driver: Send + 'static {
    fn drive(&mut self, _: Event<'_>, _: &mut Context) {}
}

pub enum Request {
    /// Raw message to be sent to the service implementation.
    ServicePayload { bytes: Bytes },
    /// Request access token from.
    AccessToken { ttl: u64 },
    /// Extend the access token associated with this connection.
    ExtendAccessToken { ttl: u64 },
}

pub enum Event<'a> {
    Connection { success: bool },
    PayloadReceived { bytes: &'a [u8] },
    Disconnect { reason: Option<TerminationReason> },
}

pub struct RequestResponse {
    pub payload: Request,
    pub respond: oneshot::Sender<Response>,
}

pub struct Handle(Sender<RequestResponse>);

impl Handle {
    pub(crate) fn new(sender: Sender<RequestResponse>) -> Self {
        Self(sender)
    }

    pub async fn send(&self, request: Request) -> Result<Response> {
        let (respond_tx, respond_rx) = oneshot::channel();
        let inner_request = RequestResponse {
            payload: request,
            respond: respond_tx,
        };
        self.0
            .send(inner_request)
            .await
            .map_err(|_| anyhow::anyhow!("failed to send request"))?;

        // Todo: can probably make this value configurable.
        let timeout = tokio::time::sleep(Duration::from_secs(3));
        tokio::select! {
            _ = timeout => {
                bail!("timeout");
            }
            response = respond_rx => {
                response.map_err(|e| anyhow::anyhow!("failed to receive response: {e:?}"))
            }
        }
    }

    pub fn disconnect(self) {
        // Dropping the senders will cause the connection to disconnect.
    }
}

// RequestFrame is internal and includes frames
// that should not be exposed to clients.
impl From<Request> for RequestFrame {
    fn from(value: Request) -> Self {
        match value {
            Request::ServicePayload { bytes } => RequestFrame::ServicePayload { bytes },
            Request::AccessToken { ttl } => RequestFrame::AccessToken { ttl },
            Request::ExtendAccessToken { ttl } => RequestFrame::ExtendAccessToken { ttl },
        }
    }
}
