use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;

#[rpc(client, server, namespace = "admin")]
pub trait AdminApi {
    #[method(name = "store")]
    async fn store(&self, path: String) -> RpcResult<()>;
}
