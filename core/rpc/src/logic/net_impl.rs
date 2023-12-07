use std::sync::Arc;

use jsonrpsee::core::RpcResult;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::SyncQueryRunnerInterface;
use tracing::trace;

use crate::api::NetApiServer;
use crate::Data;

pub struct NetApi<C: Collection> {
    data: Arc<Data<C>>,
}

impl<C: Collection> NetApi<C> {
    pub(crate) fn new(data: Arc<Data<C>>) -> Self {
        Self { data }
    }
}

#[async_trait::async_trait]
impl<C: Collection> NetApiServer for NetApi<C> {
    /// todo!()
    async fn peer_count(&self) -> RpcResult<Option<String>> {
        // todo(dalton): Figure out what this is used for in ethereum instead of just mocking it
        // here. Shouldnt be relevent for us but may need to return something here for
        // compatability
        Ok(Some("0x40".into()))
    }

    async fn version(&self) -> RpcResult<Option<String>> {
        trace!(target: "rpc::eth", "Serving net_version");
        Ok(Some(self.data.query_runner.get_chain_id().to_string()))
    }

    async fn listening(&self) -> RpcResult<bool> {
        Ok(true)
    }
}
