use fleek_crypto::ClientPublicKey;

use crate::ipc::send_and_await_response;
use crate::ipc_types::{Request, StaticVec};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    IPFS,
}

/// Returns the balance of a client with the following public key.
pub async fn query_client_bandwidth_balance(pk: ClientPublicKey) -> u128 {
    let req = Request::QueryClientBandwidth { pk: pk.0 };
    let res = send_and_await_response(req).await;
    match res {
        crate::ipc_types::Response::QueryClientBandwidth { balance } => balance,
        _ => unreachable!(),
    }
}

pub async fn query_client_flk_balance(pk: ClientPublicKey) -> u128 {
    let req = Request::QueryClientFLK { pk: pk.0 };
    let res = send_and_await_response(req).await;
    match res {
        crate::ipc_types::Response::QueryClientFLK { balance } => balance,
        _ => unreachable!(),
    }
}

pub async fn fetch_from_origin(origin: Origin, uri: impl AsRef<[u8]>) -> Option<[u8; 32]> {
    let req = Request::FetchFromOrigin {
        origin: origin as u8,
        uri: StaticVec::new(uri.as_ref()),
    };
    let res = send_and_await_response(req).await;
    match res {
        crate::ipc_types::Response::FetchFromOrigin { hash } => hash,
        _ => unreachable!(),
    }
}

pub async fn fetch_blake3(hash: [u8; 32]) -> bool {
    let req = Request::FetchBlake3 { hash };
    let res = send_and_await_response(req).await;
    match res {
        crate::ipc_types::Response::FetchBlake3 { succeeded } => succeeded,
        _ => unreachable!(),
    }
}
