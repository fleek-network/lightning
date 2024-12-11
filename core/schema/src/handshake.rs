//! TODO: unify [`LightningMessage`] with these fixed length implementations

use anyhow::{anyhow, Result};
use arrayref::array_ref;
use bytes::{BufMut, Bytes};
use fleek_crypto::{ClientPublicKey, ClientSignature, NodePublicKey, NodeSignature};
use sha2::{Digest, Sha256};

pub const NETWORK_PREFIX: &[u8; 5] = b"FLEEK";

pub const HANDSHAKE_REQ_TAG: u8 = 0x00;
pub const HANDSHAKE_RETRY_REQ_TAG: u8 = 0x01;
pub const HANDSHAKE_JOIN_REQ_TAG: u8 = 0x02;

pub const REQ_SERVICE_PAYLOAD_TAG: u8 = 0x00;
pub const REQ_ACCESS_TOKEN_TAG: u8 = 0x01;
pub const REQ_EXTEND_ACCESS_TOKEN_TAG: u8 = 0x02;
pub const REQ_DELIVERY_ACK_TAG: u8 = 0x03;

pub const RES_SERVICE_PAYLOAD_TAG: u8 = 0x00;
pub const RES_SERVICE_PAYLOAD_CHUNK_TAG: u8 = 0x40;
pub const RES_ACCESS_TOKEN_TAG: u8 = 0x01;

/// Challenge sent by the server for the client to sign in their handshake request.
/// TODO: Determine if the extra round trip is ideal here, and identify other
/// solutions for safely determining some bytes for the client proof of possession.
#[derive(Debug, PartialEq, Eq)]
pub struct ChallengeFrame {
    pub challenge: [u8; 32],
}

impl ChallengeFrame {
    pub fn encode(&self) -> Bytes {
        let mut buf = Vec::with_capacity(37);
        buf.put_slice(NETWORK_PREFIX);
        buf.put_slice(&self.challenge);
        buf.into()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 37 {
            return Err(anyhow!("wrong number of bytes"));
        }
        if array_ref!(bytes, 0, 5) != NETWORK_PREFIX {
            return Err(anyhow!("invalid network prefix"));
        }
        Ok(Self {
            challenge: *array_ref!(bytes, 5, 32),
        })
    }
}

/// Handshake frame sent by the client to either initialize a new connection or join an existing
/// one.
#[derive(Debug, PartialEq, Eq)]
pub enum HandshakeRequestFrame {
    /// Primary connection handshake.
    Handshake {
        retry: Option<u64>,
        service: u32,
        expiry: u64,
        nonce: u128,
        pk: ClientPublicKey,
        pop: ClientSignature,
    },
    /// Secondary connection join request.
    JoinRequest { access_token: [u8; 48] },
}

/// Compute a digest for signing a handshake request
pub fn handshake_digest(
    retry: Option<u64>,
    service: u32,
    expiry: u64,
    nonce: u128,
    pk: ClientPublicKey,
) -> [u8; 32] {
    let mut hasher = Sha256::default();

    hasher.update(b"RETRY");
    hasher.update([retry.is_some() as u8]);
    if let Some(id) = retry {
        hasher.update(id.to_be_bytes());
    }

    hasher.update(b"SERVICE");
    hasher.update(service.to_be_bytes());

    hasher.update(b"EXPIRY");
    hasher.update(expiry.to_be_bytes());

    hasher.update(b"NONCE");
    hasher.update(nonce.to_be_bytes());

    hasher.update(b"PUBKEY");
    hasher.update(pk.0);

    hasher.finalize().into()
}

impl HandshakeRequestFrame {
    /// Encode the frame into bytes.
    pub fn encode(&self) -> Bytes {
        match self {
            HandshakeRequestFrame::Handshake {
                retry,
                service,
                expiry,
                nonce,
                pk,
                pop,
            } => {
                let mut buf = match retry {
                    None => {
                        let mut buf = Vec::with_capacity(173);
                        buf.put_u8(HANDSHAKE_REQ_TAG);
                        buf
                    },
                    Some(id) => {
                        let mut buf = Vec::with_capacity(181);
                        buf.put_u8(HANDSHAKE_RETRY_REQ_TAG);
                        buf.put_u64(*id);
                        buf
                    },
                };
                buf.put_u32(*service);
                buf.put_u64(*expiry);
                buf.put_u128(*nonce);
                buf.put_slice(&pk.0);
                buf.put_slice(&pop.0);
                buf.into()
            },
            HandshakeRequestFrame::JoinRequest { access_token } => {
                let mut buf = Vec::with_capacity(49);
                buf.put_u8(HANDSHAKE_JOIN_REQ_TAG);
                buf.put_slice(access_token);
                buf.into()
            },
        }
    }

    /// Decode a complete buffer into a frame.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() {
            return Err(anyhow!("cannot decode empty buffer"));
        }

        match bytes[0] {
            HANDSHAKE_REQ_TAG => {
                if bytes.len() != 173 {
                    return Err(anyhow!("wrong number of bytes"));
                }
                let service = u32::from_be_bytes(*array_ref!(bytes, 1, 4));
                let expiry = u64::from_be_bytes(*array_ref![bytes, 5, 8]);
                let nonce = u128::from_be_bytes(*array_ref![bytes, 13, 16]);
                let pk = ClientPublicKey(*array_ref!(bytes, 29, 96));
                let pop = ClientSignature(*array_ref!(bytes, 125, 48));
                Ok(Self::Handshake {
                    retry: None,
                    service,
                    expiry,
                    nonce,
                    pk,
                    pop,
                })
            },
            HANDSHAKE_RETRY_REQ_TAG => {
                if bytes.len() != 181 {
                    return Err(anyhow!("wrong number of bytes"));
                }
                let retry = Some(u64::from_be_bytes(*array_ref!(bytes, 1, 8)));

                let service = u32::from_be_bytes(*array_ref!(bytes, 9, 4));
                let expiry = u64::from_be_bytes(*array_ref![bytes, 13, 8]);
                let nonce = u128::from_be_bytes(*array_ref![bytes, 21, 16]);
                let pk = ClientPublicKey(*array_ref!(bytes, 37, 96));
                let pop = ClientSignature(*array_ref!(bytes, 133, 48));
                Ok(Self::Handshake {
                    retry,
                    service,
                    expiry,
                    nonce,
                    pk,
                    pop,
                })
            },
            HANDSHAKE_JOIN_REQ_TAG => {
                if bytes.len() != 49 {
                    return Err(anyhow!("wrong number of bytes"));
                }
                let access_token = *array_ref!(bytes, 1, 48);
                Ok(Self::JoinRequest { access_token })
            },
            _ => Err(anyhow!("invalid frame tag")),
        }
    }
}

/// Server response proving the node's identity.
#[derive(Debug, PartialEq, Eq)]
pub struct HandshakeResponse {
    pub pk: NodePublicKey,
    pub pop: NodeSignature,
}

impl HandshakeResponse {
    pub fn encode(&self) -> Bytes {
        let mut buf = Vec::with_capacity(96);
        buf.put_slice(&self.pk.0);
        buf.put_slice(&self.pop.0);
        buf.into()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 96 {
            return Err(anyhow!("wrong number of bytes"));
        }

        let pk = NodePublicKey(*array_ref!(bytes, 0, 32));
        let pop = NodeSignature(*array_ref!(bytes, 32, 64));
        Ok(Self { pk, pop })
    }
}

/// Request frames sent by the client and received by the server.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RequestFrame {
    /// Raw message to be sent to the service implementation. Available for any connection level.
    ServicePayload { bytes: bytes::Bytes },
    /// Client request for an access token. Should only be used by the primary connection.
    AccessToken { ttl: u64 },
    /// Extend the access token associated with this primary connection.
    ExtendAccessToken { ttl: u64 },
    /// Delivery acknowledgment, a client signature for some work the node and
    /// service committed to.
    DeliveryAcknowledgment {
        // TODO:
    },
}

impl RequestFrame {
    /// Encode a complete frame into bytes
    pub fn encode(&self) -> Bytes {
        match self {
            Self::ServicePayload { bytes } => {
                let mut buf = Vec::new();
                buf.put_u8(REQ_SERVICE_PAYLOAD_TAG);
                buf.put_slice(bytes);
                buf.into()
            },
            Self::AccessToken { ttl } => {
                let mut buf = Vec::with_capacity(9);
                buf.put_u8(REQ_ACCESS_TOKEN_TAG);
                buf.put_u64(*ttl);
                buf.into()
            },
            Self::ExtendAccessToken { ttl } => {
                let mut buf = Vec::with_capacity(9);
                buf.put_u8(REQ_EXTEND_ACCESS_TOKEN_TAG);
                buf.put_u64(*ttl);
                buf.into()
            },
            // TODO: encode signature bytes
            Self::DeliveryAcknowledgment { .. } => vec![REQ_DELIVERY_ACK_TAG].into(),
        }
    }

    /// Decode a complete buffer of bytes into a frame.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() {
            return Err(anyhow!("cannot decode empty buffer"));
        }

        match bytes[0] {
            REQ_SERVICE_PAYLOAD_TAG => {
                let bytes = bytes[1..].to_vec().into();
                Ok(Self::ServicePayload { bytes })
            },
            REQ_ACCESS_TOKEN_TAG => {
                if bytes.len() != 9 {
                    return Err(anyhow!("wrong number of bytes"));
                }

                let ttl = u64::from_be_bytes(*array_ref!(bytes, 1, 8));
                Ok(Self::AccessToken { ttl })
            },
            REQ_EXTEND_ACCESS_TOKEN_TAG => {
                if bytes.len() != 9 {
                    return Err(anyhow!("wrong number of bytes"));
                }

                let ttl = u64::from_be_bytes(*array_ref!(bytes, 1, 8));
                Ok(Self::ExtendAccessToken { ttl })
            },
            // TODO: decode signature bytes
            REQ_DELIVERY_ACK_TAG => Ok(Self::DeliveryAcknowledgment {}),
            _ => Err(anyhow!("invalid frame tag")),
        }
    }
}

/// Response frames sent by the server and received by the client.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResponseFrame {
    /// Message to be passed.
    ServicePayload { bytes: bytes::Bytes },
    /// A chunk of a message that should be buffered and combined with the next payload frame.
    ///
    /// This frame is *never* used by stream based transports.
    ServicePayloadChunk { bytes: bytes::Bytes },
    /// Access token granted for secondary connections.
    AccessToken {
        ttl: u64,
        access_token: Box<[u8; 48]>,
    },
    /// Termination signal to gracefully end a connection with a reason.
    Termination { reason: TerminationReason },
}

impl ResponseFrame {
    /// Encode the response frame into bytes.
    pub fn encode(&self) -> Bytes {
        match self {
            Self::ServicePayload { bytes } => {
                let mut buf = Vec::with_capacity(1 + bytes.len());
                buf.put_u8(RES_SERVICE_PAYLOAD_TAG);
                buf.put_slice(bytes);
                buf.into()
            },
            Self::ServicePayloadChunk { bytes } => {
                let mut buf = Vec::with_capacity(1 + bytes.len());
                buf.put_u8(RES_SERVICE_PAYLOAD_CHUNK_TAG);
                buf.put_slice(bytes);
                buf.into()
            },
            Self::AccessToken { ttl, access_token } => {
                let mut buf = Vec::with_capacity(57);
                buf.put_u8(RES_ACCESS_TOKEN_TAG);
                buf.put_u64(*ttl);
                buf.put_slice(access_token.as_slice());
                buf.into()
            },
            Self::Termination { reason } => vec![*reason as u8].into(),
        }
    }

    /// Decode a complete buffer into a frame.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() {
            return Err(anyhow!("cannot decode empty buffer"));
        }

        match bytes[0] {
            RES_SERVICE_PAYLOAD_TAG => {
                let bytes = bytes[1..].to_vec().into();
                Ok(Self::ServicePayload { bytes })
            },
            RES_SERVICE_PAYLOAD_CHUNK_TAG => {
                let bytes = bytes[1..].to_vec().into();
                Ok(Self::ServicePayloadChunk { bytes })
            },
            RES_ACCESS_TOKEN_TAG => {
                if bytes.len() != 57 {
                    return Err(anyhow!("wrong number of bytes"));
                }
                let ttl = u64::from_be_bytes(*array_ref!(bytes, 1, 8));
                let access_token = Box::new(*array_ref!(bytes, 9, 48));
                Ok(Self::AccessToken { ttl, access_token })
            },
            byte if byte >= 0x80 => {
                if bytes.len() > 1 {
                    return Err(anyhow!("too many bytes"));
                }
                Ok(Self::Termination {
                    reason: TerminationReason::from_u8(byte),
                })
            },
            byte => Err(anyhow!("invalid frame tag: {byte}")),
        }
    }
}

/// Termination signals
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
#[non_exhaustive]
pub enum TerminationReason {
    Timeout = 0x80,
    InvalidHandshake,
    InvalidToken,
    InvalidDeliveryAcknowledgment,
    InvalidService,
    ServiceTerminated,
    ConnectionInUse,
    WrongPermssion,
    ResourcesUnavailable,
    InternalError,
    Shutdown,
    Unknown = 0xFF,
}

impl TerminationReason {
    /// Parse a byte into a termination reason.
    pub fn from_u8(byte: u8) -> Self {
        match byte {
            0x80 => Self::Timeout,
            0x81 => Self::InvalidHandshake,
            0x82 => Self::InvalidToken,
            0x83 => Self::InvalidDeliveryAcknowledgment,
            0x84 => Self::InvalidService,
            0x85 => Self::ServiceTerminated,
            0x86 => Self::ConnectionInUse,
            0x87 => Self::WrongPermssion,
            _ => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! encode_decode {
        ($t:ident, $($s:expr),*) => {
            $(
            let frame = $s;
            assert_eq!(
                frame,
                $t::decode(&frame.encode()).unwrap()
            );)*
        };
    }

    #[test]
    fn handshake_frames() {
        encode_decode!(ChallengeFrame, ChallengeFrame { challenge: [0; 32] });
        encode_decode!(
            HandshakeRequestFrame,
            HandshakeRequestFrame::Handshake {
                retry: None,
                service: 1,
                expiry: 0,
                nonce: 0,
                pk: ClientPublicKey([2; 96]),
                pop: ClientSignature([3; 48]),
            },
            HandshakeRequestFrame::Handshake {
                retry: Some(4),
                service: 5,
                expiry: 0,
                nonce: 0,
                pk: ClientPublicKey([6; 96]),
                pop: ClientSignature([7; 48]),
            },
            HandshakeRequestFrame::JoinRequest {
                access_token: [8; 48],
            }
        );
        encode_decode!(
            HandshakeResponse,
            HandshakeResponse {
                pk: NodePublicKey([9; 32]),
                pop: NodeSignature([0; 64]),
            }
        );
    }

    #[test]
    fn request_frames() {
        encode_decode!(
            RequestFrame,
            RequestFrame::ServicePayload {
                bytes: vec![1; 64].into(),
            },
            RequestFrame::AccessToken { ttl: 2 },
            RequestFrame::ExtendAccessToken { ttl: 12 },
            RequestFrame::DeliveryAcknowledgment {}
        );
    }

    #[test]
    fn response_frames() {
        encode_decode!(
            ResponseFrame,
            ResponseFrame::ServicePayload {
                bytes: vec![1; 64].into(),
            },
            ResponseFrame::ServicePayloadChunk {
                bytes: vec![1; 64].into(),
            },
            ResponseFrame::AccessToken {
                ttl: 2,
                access_token: [3; 48].into(),
            },
            ResponseFrame::Termination {
                reason: TerminationReason::Timeout
            },
            ResponseFrame::Termination {
                reason: TerminationReason::InvalidHandshake
            },
            ResponseFrame::Termination {
                reason: TerminationReason::Timeout
            },
            ResponseFrame::Termination {
                reason: TerminationReason::InvalidHandshake
            },
            ResponseFrame::Termination {
                reason: TerminationReason::InvalidToken
            },
            ResponseFrame::Termination {
                reason: TerminationReason::InvalidDeliveryAcknowledgment
            },
            ResponseFrame::Termination {
                reason: TerminationReason::InvalidService
            },
            ResponseFrame::Termination {
                reason: TerminationReason::ServiceTerminated
            },
            ResponseFrame::Termination {
                reason: TerminationReason::Unknown
            }
        );
    }
}
