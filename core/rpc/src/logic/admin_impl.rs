use std::sync::Arc;

use jsonrpsee::core::RpcResult;
use lightning_firewall::{CommandCenter, FirewallCommand};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::Blake3Hash;
use lightning_interfaces::FileTrustedWriter;

use crate::api::AdminApiServer;
use crate::error::RPCError;
use crate::Data;

pub struct AdminApi<C: NodeComponents> {
    data: Arc<Data<C>>,
}

impl<C: NodeComponents> AdminApi<C> {
    pub(crate) fn new(data: Arc<Data<C>>) -> Self {
        Self { data }
    }
}

#[async_trait::async_trait]
impl<C: NodeComponents> AdminApiServer for AdminApi<C> {
    async fn store(&self, path: String) -> RpcResult<Blake3Hash> {
        let file = tokio::fs::read(path)
            .await
            .map_err(|e| RPCError::custom(e.to_string()))?;

        let mut writer = self
            .data
            ._blockstore
            .file_writer()
            .await
            .map_err(|e| RPCError::custom(e.to_string()))?;
        writer
            .write(file.as_ref(), true)
            .await
            .map_err(|e| RPCError::custom(e.to_string()))?;
        let hash = writer
            .commit()
            .await
            .map_err(|e| RPCError::custom(e.to_string()))?;
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

    async fn ping(&self) -> RpcResult<String> {
        Ok("pong".to_string())
    }
}
