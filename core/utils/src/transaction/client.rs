use std::collections::HashSet;
use std::time::Duration;

use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    TransactionReceipt,
    TransactionRequest,
    TransactionResponse,
    UpdateMethod,
};
use tokio::sync::oneshot;

use super::listener::TransactionReceiptListener;
use super::{TransactionBuilder, TransactionClientError, TransactionSigner};
use crate::application::QueryRunnerExt;

pub struct ExecuteTransactionOptions {
    pub wait_for_receipt_timeout: Duration,
    pub retry_on_revert: HashSet<TransactionResponse>,
    pub retry_on_revert_delay: Duration,
    pub execution_timeout: Duration,
}

/// A client for submitting and executing transactions, and waiting for their receipts.
///
/// The client is signer-specific, and will sign the incoming transaction with the configured signer
/// before submitting it.
pub struct TransactionClient<C: NodeComponents> {
    app_query: c!(C::ApplicationInterface::SyncExecutor),
    mempool: MempoolSocket,
    signer: TransactionSigner,
    listener: TransactionReceiptListener<C>,
}

impl<C: NodeComponents> TransactionClient<C> {
    pub async fn new(
        app_query: c!(C::ApplicationInterface::SyncExecutor),
        notifier: C::NotifierInterface,
        mempool: MempoolSocket,
        signer: TransactionSigner,
    ) -> Self {
        let listener = TransactionReceiptListener::spawn(notifier).await;

        Self {
            app_query,
            mempool,
            signer,
            listener,
        }
    }

    /// Submit an update request to the application executor and wait for it to be executed. Returns
    /// the transaction request and its receipt.
    ///
    /// If the transaction is not executed within a timeout, an error is returned.
    pub async fn execute_transaction(
        &self,
        method: UpdateMethod,
    ) -> Result<(TransactionRequest, TransactionReceipt), TransactionClientError> {
        let (tx, receipt) = self
            .execute_transaction_with_options(
                method,
                ExecuteTransactionOptions {
                    wait_for_receipt_timeout: Duration::from_secs(30),
                    retry_on_revert: HashSet::new(),
                    retry_on_revert_delay: Duration::from_millis(100),
                    execution_timeout: Duration::from_secs(30),
                },
            )
            .await?;
        match receipt.response {
            TransactionResponse::Success(_) => Ok((tx, receipt)),
            TransactionResponse::Revert(_) => Err(TransactionClientError::Reverted((tx, receipt))),
        }
    }

    /// Submit an update request to the application executor and wait for it to be executed. Returns
    /// the transaction request and its receipt.
    ///
    /// This method also accepts options to configure the behavior of the transaction execution,
    /// such as the timeout for waiting for the transaction receipt, and the retry behavior for
    /// reverted transactions.
    ///
    /// If the transaction is not executed within a timeout, an error is returned.
    pub async fn execute_transaction_with_options(
        &self,
        method: UpdateMethod,
        options: ExecuteTransactionOptions,
    ) -> Result<(TransactionRequest, TransactionReceipt), TransactionClientError> {
        let chain_id = self.app_query.get_chain_id();
        let start = tokio::time::Instant::now();
        loop {
            // Build and sign the transaction.
            let nonce = self.signer.get_nonce(&self.app_query);
            let tx: TransactionRequest =
                TransactionBuilder::from_update(method.clone(), chain_id, nonce + 1, &self.signer)
                    .into();

            // If we've timed out, return an error.
            if start.elapsed() >= options.execution_timeout {
                return Err(TransactionClientError::Timeout(tx));
            }

            // Register transaction with pending transactions listener.
            let receipt_rx = self.listener.register(tx.hash()).await;

            // Send transaction to the mempool.
            self.mempool
                .enqueue(tx.clone())
                .await
                .map_err(TransactionClientError::MempoolSendFailed)?;

            // Wait for the transaction to be executed, and return the receipt.
            let receipt = self
                .wait_for_receipt(tx.clone(), receipt_rx, options.wait_for_receipt_timeout)
                .await?;

            // If the transaction was reverted, and retry is enabled for this type of revert, sleep
            // for a short period and retry the transaction.
            if options.retry_on_revert.contains(&receipt.response) {
                tracing::info!(
                    "retrying reverted transaction (hash: {:?}): {:?}",
                    tx.hash(),
                    receipt.response
                );
                tokio::time::sleep(options.retry_on_revert_delay).await;
                continue;
            }

            // Otherwise, return success with the receipt.
            return Ok((tx, receipt));
        }
    }

    /// Wait for a transaction receipt for a given transaction.
    ///
    /// If the transaction is not executed within a timeout, an error is returned.
    async fn wait_for_receipt(
        &self,
        tx: TransactionRequest,
        receipt_rx: oneshot::Receiver<TransactionReceipt>,
        timeout: Duration,
    ) -> Result<TransactionReceipt, TransactionClientError> {
        let timeout_fut = tokio::time::sleep(timeout);
        tokio::pin!(timeout_fut);
        tokio::select! {
            result = receipt_rx => {
                let receipt = result.map_err(|e| TransactionClientError::Internal(e.to_string()))?;
                match receipt.response {
                    TransactionResponse::Success(_) => {
                        tracing::debug!("transaction executed: {:?}", receipt);
                    },
                    TransactionResponse::Revert(_) => {
                        tracing::debug!("transaction reverted: {:?}", receipt);
                    },
                }
                Ok(receipt)
            },
            _ = &mut timeout_fut => {
                tracing::debug!("timeout while waiting for transaction receipt: {:?}", tx.hash());
                Err(TransactionClientError::TimeoutWaitingForReceipt(tx))
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use lightning_interfaces::prelude::*;
    use lightning_test_utils::e2e::{TestNetwork, TestNodeComponents};
    use types::ExecutionData;

    use super::*;

    #[tokio::test]
    async fn test_execute_transaction_with_account_signer() {
        let mut network = TestNetwork::builder()
            .with_num_nodes(1)
            .build()
            .await
            .unwrap();
        let node = network.node(0);

        // Build a transaction client.
        let client = TransactionClient::<TestNodeComponents>::new(
            node.app_query.clone(),
            node.notifier.clone(),
            node.forwarder.mempool_socket(),
            TransactionSigner::AccountOwner(node.owner_secret_key.clone()),
        )
        .await;

        // Execute a transaction and wait for it to complete.
        let (tx, receipt) = client
            .execute_transaction(UpdateMethod::IncrementNonce {})
            .await
            .unwrap();
        assert_eq!(
            receipt.response,
            TransactionResponse::Success(ExecutionData::None)
        );
        assert!(!tx.hash().is_empty());
        assert_eq!(receipt.transaction_hash, tx.hash());

        // Shutdown the network.
        network.shutdown().await;
    }

    #[tokio::test]
    async fn test_execute_transaction_with_node_signer() {
        let mut network = TestNetwork::builder()
            .with_num_nodes(1)
            .build()
            .await
            .unwrap();
        let node = network.node(0);

        // Build a transaction client.
        let client = TransactionClient::<TestNodeComponents>::new(
            node.app_query.clone(),
            node.notifier.clone(),
            node.forwarder.mempool_socket(),
            TransactionSigner::NodeMain(node.keystore.get_ed25519_sk()),
        )
        .await;

        // Execute a transaction and wait for it to complete.
        let (tx, receipt) = client
            .execute_transaction(UpdateMethod::IncrementNonce {})
            .await
            .unwrap();
        assert_eq!(
            receipt.response,
            TransactionResponse::Success(ExecutionData::None)
        );
        assert!(!tx.hash().is_empty());
        assert_eq!(receipt.transaction_hash, tx.hash());

        // Shutdown the network.
        network.shutdown().await;
    }
}
