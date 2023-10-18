use derive_more::IsVariant;

use crate::ReqRes;

/// Client public key bytes. We use this type alias instead of the actual fleek-crypto type
/// to allow us to use rkyv.
pub type ClientPublicKeyBytes = [u8; 96];

pub type RequestCtxU64 = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct IpcRequest {
    /// A pointer to the request context.
    pub request_ctx: Option<RequestCtxU64>,
    /// The request to be processed by core.
    pub request: Request,
}

/// A message sent from the core process to the service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub enum IpcMessage {
    Connected {
        connection_id: u64,
        client: ClientPublicKeyBytes,
    },
    Response {
        request_ctx: RequestCtxU64,
        response: Response,
    },
}

ReqRes! {
    /// Query a client's balance.
    QueryClientBalance {
        /// The public key of the user that we want their balance.
        pk: ClientPublicKeyBytes,
        =>
        /// The balance of the user.
        balance: u128,
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
