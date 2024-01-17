use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use lightning_interfaces::types::Blake3Hash;

#[rpc(client, server, namespace = "admin")]
pub trait AdminApi {
    #[method(name = "store")]
    async fn store(&self, path: String) -> RpcResult<Blake3Hash>;
}
