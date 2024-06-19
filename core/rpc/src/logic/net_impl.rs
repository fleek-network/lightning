use std::sync::Arc;

use jsonrpsee::core::RpcResult;
use lightning_interfaces::Collection;
use tracing::trace;

use crate::api::NetApiServer;
use crate::{Data, VERSION};

pub struct NetApi<C: Collection> {
    _data: Arc<Data<C>>,
}

impl<C: Collection> NetApi<C> {
    pub(crate) fn new(data: Arc<Data<C>>) -> Self {
        Self { _data: data }
    }
}

#[async_trait::async_trait]
impl<C: Collection> NetApiServer for NetApi<C> {
    /// todo!()
    async fn peer_count(&self) -> RpcResult<Option<String>> {
        trace!(target: "rpc::eth", "Serving eth_chainId");
        Ok(Some("0x40".into()))
    }

    async fn version(&self) -> RpcResult<Option<String>> {
        Ok(Some(VERSION.clone()))
    }

    async fn listening(&self) -> RpcResult<bool> {
        Ok(true)
    }
}
