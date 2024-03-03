use std::ops::Deref;

use derive_more::IsVariant;
use lightning_schema::AutoImplSerde;
use serde::{Deserialize, Serialize};

use crate::ReqRes;

/// Client public key bytes. We use this type alias instead of the actual fleek-crypto type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClientPublicKeyBytes([u8; 96]);

impl Serialize for ClientPublicKeyBytes {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> Deserialize<'de> for ClientPublicKeyBytes {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let buffer = <&[u8]>::deserialize(deserializer)?;

        if buffer.len() != 96 {
            return Err(serde::de::Error::custom(format!(
                "Expected 96 bytes, got {}",
                buffer.len()
            )));
        }

        let mut new_buffer = [0; 96];
        new_buffer.copy_from_slice(buffer);
        Ok(Self(new_buffer))
    }
}

impl From<[u8; 96]> for ClientPublicKeyBytes {
    fn from(value: [u8; 96]) -> Self {
        Self(value)
    }
}

impl From<ClientPublicKeyBytes> for [u8; 96] {
    fn from(value: ClientPublicKeyBytes) -> Self {
        value.0
    }
}

pub type RequestCtxU64 = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticVec<const CAP: usize> {
    size: usize,
    buffer: [u8; CAP],
}

impl<const CAP: usize> Serialize for StaticVec<CAP> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(&self.buffer[0..self.size])
    }
}

impl<'de, const CAP: usize> Deserialize<'de> for StaticVec<CAP> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let buffer = <&[u8]>::deserialize(deserializer)?;
        let size = buffer.len();
        let mut new_buffer = [0; CAP];
        new_buffer[0..size].copy_from_slice(buffer);
        Ok(Self {
            size,
            buffer: new_buffer,
        })
    }
}

impl<const CAP: usize> Deref for StaticVec<CAP> {
    type Target = [u8];
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.buffer[0..self.size]
    }
}

impl<const CAP: usize> StaticVec<CAP> {
    pub fn new(src: &[u8]) -> Self {
        let size = src.len();
        assert!(size <= CAP);
        let mut buffer = [0; CAP];
        buffer[0..size].copy_from_slice(src);
        Self { size, buffer }
    }
}
impl<const CAP: usize> From<&StaticVec<CAP>> for Vec<u8> {
    fn from(value: &StaticVec<CAP>) -> Self {
        let mut vec = Vec::with_capacity(value.size);
        vec.extend_from_slice(&value[0..value.size]);
        vec
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpcRequest {
    /// A pointer to the request context.
    pub request_ctx: Option<RequestCtxU64>,
    /// The request to be processed by core.
    pub request: Request,
}

/// A message sent from the core process to the service.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IpcMessage {
    Response {
        request_ctx: RequestCtxU64,
        response: Response,
    },
}

impl AutoImplSerde for IpcRequest {}
impl AutoImplSerde for IpcMessage {}
impl AutoImplSerde for Request {}
impl AutoImplSerde for Response {}

ReqRes! {
    /// Query a client's bandwidth balance.
    QueryClientBandwidth {
        /// The public key of the user that we want their balance.
        pk: ClientPublicKeyBytes,
        =>
        /// The balance of the user.
        balance: u128,
    },
    /// Query a client's FLK balance.
    QueryClientFLK {
        /// The public key of the user.
        pk: ClientPublicKeyBytes,
        =>
        /// The balance of the user.
        balance: u128,
    },
    FetchFromOrigin {
        origin: u8,
        /// The encoded URI.
        uri: StaticVec<256>,
        =>
        /// Returns the hash of the content on successful fetch.
        hash: Option<[u8; 32]>,
    },
    /// Fetch a content via a blake3 digest.
    FetchBlake3 {
        /// Hash of the content we are interested to fetch.
        hash: [u8; 32],
        =>
        /// Returns true if the fetch succeeded.
        succeeded: bool
    },
}
