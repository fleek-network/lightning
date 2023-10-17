use fleek_crypto::ClientPublicKey;

use crate::ipc::send_and_await_response;
use crate::ipc_types::Request;

/// Returns the balance of a client with the following public key.
pub async fn query_client_balance(pk: ClientPublicKey) -> u128 {
    let req = Request::QueryClientBalance { pk: pk.0 };
    let res = send_and_await_response(req).await;
    match res {
        crate::ipc_types::Response::QueryClientBalance { balance } => balance,
        _ => unreachable!(),
    }
}
