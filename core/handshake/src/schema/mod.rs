use fleek_crypto::{ClientPublicKey, ClientSignature, NodePublicKey, NodeSignature};
use lightning_interfaces::types::ServiceId;

pub struct ChallengeFrame {
    pub challenge: [u8; 32],
}

pub enum HandshakeRequestFrame {
    Handshake {
        pk: ClientPublicKey,
        pop: ClientSignature,
        service: ServiceId,
        retry: Option<u64>,
    },
    ContinueRequest {
        access_token: [u8; 48],
    },
}

pub struct HandshakeResponse {
    pub pk: NodePublicKey,
    pub pop: NodeSignature,
}

#[non_exhaustive]
pub enum RequestFrame {
    /// Raw message to be sent to the service implementation.
    ServicePayload { bytes: bytes::Bytes },
    /// Request access token from.
    AccessToken { ttl: u64 },
    /// Refresh an access token.
    RefreshAccessToken { access_token: Box<[u8; 48]> },
    ///
    DeliveryAcknowledgment {
        // TODO
    },
}

#[non_exhaustive]
pub enum ResponseFrame {
    /// Message to be passed.
    ServicePayload {
        bytes: bytes::Bytes,
    },
    AccessToken {
        ttl: u64,
        access_token: Box<[u8; 48]>,
    },
    Termination {
        reason: TerminationReason,
    },
}

pub enum TerminationReason {
    Timeout,
    InvalidHandshake,
    InvalidToken,
    InvalidDeliveryAcknowledgment,
    InvalidService,
    ServiceTerminated,
}
