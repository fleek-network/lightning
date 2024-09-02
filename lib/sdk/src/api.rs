use fleek_crypto::{ClientPublicKey, NodeSignature};
use lightning_schema::task_broker::TaskScope;

use crate::ipc::send_and_await_response;
use crate::ipc_types::Request;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    IPFS,
    HTTP,
}

/// Returns the balance of a client with the following public key.
pub async fn query_client_bandwidth_balance(pk: ClientPublicKey) -> u128 {
    let req = Request::QueryClientBandwidth { pk: pk.0.into() };
    let res = send_and_await_response(req).await;
    match res {
        crate::ipc_types::Response::QueryClientBandwidth { balance } => balance,
        _ => unreachable!(),
    }
}

pub async fn query_client_flk_balance(pk: ClientPublicKey) -> u128 {
    let req = Request::QueryClientFLK { pk: pk.0.into() };
    let res = send_and_await_response(req).await;
    match res {
        crate::ipc_types::Response::QueryClientFLK { balance } => balance,
        _ => unreachable!(),
    }
}

pub async fn fetch_from_origin(origin: Origin, uri: impl Into<Vec<u8>>) -> Option<[u8; 32]> {
    let req = Request::FetchFromOrigin {
        origin: origin as u8,
        uri: uri.into(),
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

pub async fn fetch_peer_ips(amount: u32) -> Vec<String> {
    let req = Request::FetchPeerIps { amount };
    let res = send_and_await_response(req).await;
    match res {
        crate::ipc_types::Response::FetchPeerIps { peer_ips } => peer_ips,
        _ => unreachable!(),
    }
}

pub async fn fetch_node_index() -> Option<u32> {
    let req = Request::FetchNodeIndex {};
    let res = send_and_await_response(req).await;
    match res {
        crate::ipc_types::Response::FetchNodeIndex { node_index } => node_index,
        _ => unreachable!(),
    }
}

pub async fn run_task(
    depth: u8,
    scope: TaskScope,
    service: u32,
    payload: Vec<u8>,
) -> (Vec<Vec<u8>>, Vec<NodeSignature>) {
    let req = Request::Task {
        scope: scope.into(),
        depth,
        service,
        payload,
    };
    let res = send_and_await_response(req).await;
    match res {
        crate::ipc_types::Response::Task {
            responses,
            signatures,
        } => (
            responses,
            signatures.into_iter().map(NodeSignature).collect(),
        ),
        _ => unreachable!(),
    }
}
