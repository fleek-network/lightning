use std::sync::Arc;

use jsonrpsee::core::RpcResult;
use lightning_interfaces::{
    SyncQueryRunnerInterface,
    infu_collection::Collection
};
use tracing::trace;

use crate::api::NetApiServer;
use crate::Data;
use crate::error::RPCError;

pub struct NetApi<C: Collection> {
    data: Arc<Data<C>>,
}

impl <C: Collection> NetApi<C> {
    pub(crate) fn new(data: Arc<Data<C>>) -> Self {
        Self {
            data
        }
    }
}

#[async_trait::async_trait]
impl<C: Collection> NetApiServer for NetApi<C> {
    async fn peer_count(&self) -> RpcResult<Option<String>> {
        trace!(target: "rpc::eth", "Serving eth_chainId");
        Ok(Some(self.data.query_runner.get_chain_id().to_string()))
    }

    async fn version(&self) -> RpcResult<Option<String>> {
        Err(RPCError::unimplemented().into())
    }

    async fn listening(&self) -> RpcResult<bool> {
        Ok(true)
    }
}
