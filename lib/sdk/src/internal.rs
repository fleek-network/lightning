use std::path::PathBuf;

use derive_more::IsVariant;
use fleek_crypto::ClientPublicKey;

use crate::ReqRes;

pub struct IpcRequest {
    /// A pointer to the request context.
    pub request_ctx: *mut u8,
    /// The request to be processed by core.
    pub request: Request,
}

/// A request id that we sent out.
#[repr(C)]
pub struct OnStartArgs {
    /// The function which should be used to send requests from.
    pub request_sender: Box<dyn Fn(IpcRequest)>,
    /// The OS path to the blockstore. The service is allowed to
    /// read content from this path.
    pub block_store_path: PathBuf,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct OnConnectedArgs {
    pub connection_id: u64,
    pub client: ClientPublicKey,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct OnDisconnectedArgs {
    pub connection_id: u64,
}

#[derive(Clone, Debug)]
#[repr(C)]
pub struct OnMessageArgs {
    pub connection_id: u64,
    pub payload: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct OnEventResponseArgs {
    pub request_ctx: *mut u8,
    pub response: Response,
}

ReqRes! {
    /// Query a client's balance.
    QueryClientBalance {
        /// The public key of the user that we want their balance.
        pk: ClientPublicKey,
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
    }
}
