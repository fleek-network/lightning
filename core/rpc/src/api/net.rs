use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;

use lightning_openrpc_macros::open_rpc;

#[open_rpc(namespace = "net", tag = "1.0.0")]
#[rpc(client, server, namespace = "net")]
pub trait NetApi {
    #[method(name = "peerCount")]
    async fn peer_count(&self) -> RpcResult<Option<String>>;

    #[method(name = "version")]
    async fn version(&self) -> RpcResult<Option<String>>;

    #[method(name = "listening")]
    async fn listening(&self) -> RpcResult<bool>;
}
