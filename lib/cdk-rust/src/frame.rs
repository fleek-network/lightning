use bytes::Bytes;

use crate::schema::{RequestFrame, ResponseFrame};

pub type Response = ResponseFrame;

pub enum Request {
    /// Raw message to be sent to the service implementation.
    ServicePayload { bytes: Bytes },
    /// Request access token from.
    AccessToken { ttl: u64 },
    /// Extend the access token associated with this connection.
    ExtendAccessToken { ttl: u64 },
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
