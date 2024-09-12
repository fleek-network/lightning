use std::time::Duration;

use anyhow::Result;
use fleek_crypto::SecretKey;
use lightning_interfaces::types::{TransactionRequest, UpdateMethod, UpdatePayload, UpdateRequest};
use lightning_interfaces::{
    ApplicationInterface,
    ForwarderInterface,
    SyncQueryRunnerInterface,
    ToDigest,
};
use lightning_utils::application::QueryRunnerExt;

use super::{wait_until, TestNode};

impl TestNode {
    pub async fn run_transaction(&self, transaction: TransactionRequest) -> Result<()> {
        // Submit transaction to mempool via forwarder.
        self.forwarder
            .mempool_socket()
            .run(transaction.clone())
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        // Wait for transaction to be executed.
        wait_until(
            || async {
                self.app
                    .sync_query()
                    .has_executed_digest(transaction.hash())
                    .then_some(())
            },
            Duration::from_secs(6),
            Duration::from_millis(200),
        )
        .await?;

        Ok(())
    }

    pub fn new_update_transaction(&self, method: UpdateMethod) -> TransactionRequest {
        let node_info = self.get_node_info().expect("node not found");
        let payload = UpdatePayload {
            sender: self.get_node_secret_key().to_pk().into(),
            nonce: node_info.nonce + 1,
            method,
            chain_id: self.app.sync_query().get_chain_id(),
        };
        let digest = payload.to_digest();
        let signature = self.get_node_secret_key().sign(&digest);

        TransactionRequest::UpdateRequest(UpdateRequest {
            signature: signature.into(),
            payload,
        })
    }
}
