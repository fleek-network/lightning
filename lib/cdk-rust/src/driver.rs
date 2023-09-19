use std::borrow::Cow;

use anyhow::Result;
use bytes::Bytes;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

use crate::context::Context;
use crate::schema::{RequestFrame, ResponseFrame, TerminationReason};

pub type Response = ResponseFrame;

pub trait Driver {
    fn drive<'a>(&mut self, _: Event<'a>, _: &mut Context) {}
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
    pub fn send(&self) -> Result<Response> {
        todo!()
    }

    pub fn disconnect(self) -> Result<()> {
        todo!()
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
