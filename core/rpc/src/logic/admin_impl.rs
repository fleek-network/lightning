use std::sync::Arc;

use jsonrpsee::core::RpcResult;
use lightning_firewall::{CommandCenter, FirewallCommand};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{Blake3Hash, CompressionAlgorithm};

use crate::api::AdminApiServer;
use crate::error::RPCError;
use crate::Data;

pub struct AdminApi<C: Collection> {
    data: Arc<Data<C>>,
}

impl<C: Collection> AdminApi<C> {
    pub(crate) fn new(data: Arc<Data<C>>) -> Self {
        Self { data }
    }
}

#[async_trait::async_trait]
impl<C: Collection> AdminApiServer for AdminApi<C> {
    async fn store(&self, path: String) -> RpcResult<Blake3Hash> {
        let file = tokio::fs::read(path)
            .await
            .map_err(|e| RPCError::custom(e.to_string()))?;

        let mut putter = self.data._blockstore.put(None);
        putter
            .write(file.as_ref(), CompressionAlgorithm::Uncompressed)
            .map_err(|e| RPCError::custom(format!("failed to write content: {e}")))?;
        let hash = putter
            .finalize()
            .await
            .map_err(|e| RPCError::custom(format!("failed to finalize put: {e}")))?;

        Ok(hash)
    }

    /// Queue a firewall command to be executed, doesnt wait for a resposne
    async fn queue_firewall_command(&self, name: &str, command: FirewallCommand) -> RpcResult<()> {
        let tx = CommandCenter::global()
            .sender(name)
            .ok_or_else(|| RPCError::custom("firewall not found".to_string()))?;

        command
            .send(&tx)
            .await
            .map_err(|e| RPCError::custom(e.to_string()))?;

        Ok(())
    }

    /// Queue a firewall command to be executed, waits for a response
    async fn queue_firewall_command_and_wait_for_success(
        &self,
        name: &str,
        command: FirewallCommand,
    ) -> RpcResult<()> {
        let tx = CommandCenter::global()
            .sender(name)
            .ok_or_else(|| RPCError::custom("firewall not found".to_string()))?;

        command
            .send_and_wait(&tx)
            .await
            .map_err(|e| RPCError::custom(e.to_string()))?;

        Ok(())
    }
}
