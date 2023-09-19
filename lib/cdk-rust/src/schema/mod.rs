#![allow(unused)]
// Todo: This is only temporary.
// Consolidate these definitions with the schemas in core/handshake.

use anyhow::{anyhow, Result};
use arrayref::array_ref;
use bytes::{BufMut, Bytes};
use fleek_crypto::{ClientPublicKey, ClientSignature, NodePublicKey, NodeSignature};
use tokio::io::{AsyncRead, AsyncReadExt};

#[derive(Debug, PartialEq, Eq)]
pub enum HandshakeRequestFrame {
    Handshake {
        retry: Option<u64>,
        service: u32, // ServiceId from interfaces.
        pk: ClientPublicKey,
        pop: ClientSignature,
    },
    JoinRequest {
        access_token: [u8; 48],
    },
}

impl HandshakeRequestFrame {
    pub fn encode(&self) -> Bytes {
        match self {
            HandshakeRequestFrame::Handshake {
                retry,
                service,
                pk,
                pop,
            } => {
                let mut buf = match retry {
                    None => {
                        let mut buf = Vec::with_capacity(149);
                        buf.put_u8(0x00);
                        buf
                    },
                    Some(id) => {
                        let mut buf = Vec::with_capacity(157);
                        buf.put_u8(0x01);
                        buf.put_u64(*id);
                        buf
                    },
                };
                buf.put_u32(*service);
                buf.put_slice(&pk.0);
                buf.put_slice(&pop.0);
                buf.into()
            },
            HandshakeRequestFrame::JoinRequest { access_token } => {
                let mut buf = Vec::with_capacity(49);
                buf.put_u8(0x02);
                buf.put_slice(access_token);
                buf.into()
            },
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        match bytes[0] {
            0x00 => {
                if bytes.len() != 149 {
                    return Err(anyhow!("wrong number of bytes"));
                }
                let service = u32::from_be_bytes(*array_ref!(bytes, 1, 4));
                let pk = ClientPublicKey(*array_ref!(bytes, 5, 96));
                let pop = ClientSignature(*array_ref!(bytes, 101, 48));
                Ok(Self::Handshake {
                    pk,
                    pop,
                    service,
                    retry: None,
                })
            },
            0x01 => {
                if bytes.len() != 157 {
                    return Err(anyhow!("wrong number of bytes"));
                }
                let retry = Some(u64::from_be_bytes(*array_ref!(bytes, 1, 8)));
                let service = u32::from_be_bytes(*array_ref!(bytes, 9, 4));
                let pk = ClientPublicKey(*array_ref!(bytes, 13, 96));
                let pop = ClientSignature(*array_ref!(bytes, 109, 48));
                Ok(Self::Handshake {
                    retry,
                    service,
                    pk,
                    pop,
                })
            },
            0x02 => {
                if bytes.len() != 49 {
                    return Err(anyhow!("wrong number of bytes"));
                }
                let access_token = *array_ref!(bytes, 1, 48);
                Ok(Self::JoinRequest { access_token })
            },
            _ => Err(anyhow!("invalid frame tag")),
        }
    }

    pub async fn decode_from_reader<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Self> {
        let ty = reader.read_u8().await?;
        match ty {
            0x00 => {
                let mut buf = vec![0u8; 149];
                reader
                    .read_exact(buf.get_mut(1..).expect("Buffer is large enough"))
                    .await?;
                buf[0] = 0x00;
                Self::decode(&buf)
            },
            0x01 => {
                let mut buf = vec![0u8; 157];
                reader
                    .read_exact(buf.get_mut(1..).expect("Buffer is large enough"))
                    .await?;
                buf[0] = 0x01;
                Self::decode(&buf)
            },
            0x02 => {
                let mut buf = vec![0u8; 49];
                reader
                    .read_exact(buf.get_mut(1..).expect("Buffer is large enough"))
                    .await?;
                buf[0] = 0x02;
                Self::decode(&buf)
            },
            _ => Err(anyhow!("invalid frame tag")),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RequestFrame {
    /// Raw message to be sent to the service implementation.
    ServicePayload { bytes: bytes::Bytes },
    /// Request access token from.
    AccessToken { ttl: u64 },
    /// Extend the access token associated with this connection.
    ExtendAccessToken { ttl: u64 },
    DeliveryAcknowledgment {
        // TODO
    },
}

impl RequestFrame {
    pub fn encode(&self) -> Bytes {
        match self {
            Self::ServicePayload { bytes } => {
                let mut buf = Vec::new();
                buf.put_u8(0x00);
                buf.put_slice(bytes);
                buf.into()
            },
            Self::AccessToken { ttl } => {
                let mut buf = Vec::with_capacity(9);
                buf.put_u8(0x01);
                buf.put_u64(*ttl);
                buf.into()
            },
            Self::ExtendAccessToken { ttl } => {
                let mut buf = Vec::with_capacity(9);
                buf.put_u8(0x02);
                buf.put_u64(*ttl);
                buf.into()
            },
            Self::DeliveryAcknowledgment {} => vec![0x03].into(),
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        match bytes[0] {
            0x00 => {
                let bytes = bytes[1..].to_vec().into();
                Ok(Self::ServicePayload { bytes })
            },
            0x01 => {
                if bytes.len() != 9 {
                    return Err(anyhow!("wrong number of bytes"));
                }

                let ttl = u64::from_be_bytes(*array_ref!(bytes, 1, 8));
                Ok(Self::AccessToken { ttl })
            },
            0x02 => {
                if bytes.len() != 9 {
                    return Err(anyhow!("wrong number of bytes"));
                }

                let ttl = u64::from_be_bytes(*array_ref!(bytes, 1, 8));
                Ok(Self::ExtendAccessToken { ttl })
            },
            0x03 => Ok(Self::DeliveryAcknowledgment {}),
            _ => Err(anyhow!("invalid frame tag")),
        }
    }

    pub async fn decode_from_reader<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Self> {
        let ty = reader.read_u8().await?;
        match ty {
            0x00 => {
                let mut bytes = Vec::new();
                reader.read_to_end(&mut bytes).await?;
                Ok(Self::ServicePayload {
                    bytes: bytes.into(),
                })
            },
            0x01 => {
                let mut bytes = vec![0u8; 8];
                reader.read_exact(&mut bytes).await?;
                let ttl = u64::from_be_bytes(*array_ref!(bytes, 0, 8));
                Ok(Self::AccessToken { ttl })
            },
            0x02 => {
                let mut bytes = vec![0u8; 8];
                reader.read_exact(&mut bytes).await?;
                let ttl = u64::from_be_bytes(*array_ref!(bytes, 0, 8));
                Ok(Self::ExtendAccessToken { ttl })
            },
            0x03 => Ok(Self::DeliveryAcknowledgment {}),
            _ => Err(anyhow!("invalid frame tag")),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
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

impl ResponseFrame {
    pub fn encode(&self) -> Bytes {
        match self {
            Self::ServicePayload { bytes } => {
                let mut buf = Vec::with_capacity(1 + bytes.len());
                buf.put_u8(0x80);
                buf.put_slice(bytes);
                buf.into()
            },
            Self::AccessToken { ttl, access_token } => {
                let mut buf = Vec::with_capacity(57);
                buf.put_u8(0x81);
                buf.put_u64(*ttl);
                buf.put_slice(access_token.as_slice());
                buf.into()
            },
            Self::Termination { reason } => vec![*reason as u8].into(),
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        match bytes[0] {
            0x80 => {
                let bytes = bytes[1..].to_vec().into();
                Ok(Self::ServicePayload { bytes })
            },
            0x81 => {
                if bytes.len() != 57 {
                    return Err(anyhow!("wrong number of bytes"));
                }
                let ttl = u64::from_be_bytes(*array_ref!(bytes, 1, 8));
                let access_token = Box::new(*array_ref!(bytes, 9, 48));
                Ok(Self::AccessToken { ttl, access_token })
            },
            byte => {
                if bytes.len() > 1 {
                    return Err(anyhow!("too many bytes"));
                }
                Ok(Self::Termination {
                    reason: TerminationReason::from_u8(byte),
                })
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
#[non_exhaustive]
pub enum TerminationReason {
    Timeout = 0x00,
    InvalidHandshake,
    InvalidToken,
    InvalidDeliveryAcknowledgment,
    InvalidService,
    ServiceTerminated,
    ConnectionInUse,
    WrongPermssion,
    Unknown = 0xFF,
}

impl TerminationReason {
    pub fn from_u8(byte: u8) -> Self {
        match byte {
            0x00 => Self::Timeout,
            0x01 => Self::InvalidHandshake,
            0x02 => Self::InvalidToken,
            0x03 => Self::InvalidDeliveryAcknowledgment,
            0x04 => Self::InvalidService,
            0x05 => Self::ServiceTerminated,
            0x06 => Self::ConnectionInUse,
            0x07 => Self::WrongPermssion,
            _ => Self::Unknown,
        }
    }
}
