use std::sync::Arc;

use jsonrpsee::core::RpcResult;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::CompressionAlgorithm;
use lightning_interfaces::{BlockStoreInterface, IncrementalPutInterface};

use crate::api::AdminApiServer;
use crate::error::RPCError;
use crate::Data;

pub struct AdminApi<C: Collection> {
    data: Arc<Data<C>>,
}

impl<C: Collection> AdminApi<C> {
    pub(crate) fn new(data: Arc<Data<C>>) -> Self {
        Self { data: data }
    }
}

#[async_trait::async_trait]
impl<C: Collection> AdminApiServer for AdminApi<C> {
    async fn store(&self, path: String) -> RpcResult<()> {
        let file = tokio::fs::read(path)
            .await
            .map_err(|e| RPCError::custom(e.to_string()))?;

        let mut putter = self.data._blockstore.put(None);
        putter
            .write(file.as_ref(), CompressionAlgorithm::Uncompressed)
            .map_err(|e| RPCError::custom(format!("failed to write content: {e}")))?;
        putter
            .finalize()
            .await
            .map_err(|e| RPCError::custom(format!("failed to finalize put: {e}")))?;
        Ok(())
    }
}
