use std::time::Duration;

use lightning_interfaces::prelude::*;
use lightning_interfaces::BlockExecutedNotification;
use types::{
    ChainId,
    ExecuteTransactionError,
    ExecuteTransactionOptions,
    ExecuteTransactionResponse,
    ExecutionError,
    TransactionReceipt,
    TransactionRequest,
    TransactionResponse,
    UpdateMethod,
};

use super::nonce::NonceState;
use super::{TransactionBuilder, TransactionSigner, DEFAULT_MAX_RETRIES, DEFAULT_TIMEOUT};
use crate::application::QueryRunnerExt;

const RETRY_DELAY: Duration = Duration::from_millis(100);

type RetryCount = u8;

pub struct TransactionRunner<C: NodeComponents> {
    notifier: C::NotifierInterface,
    mempool: MempoolSocket,
    signer: TransactionSigner,
    nonce_state: NonceState,
    chain_id: ChainId,
}

impl<C: NodeComponents> TransactionRunner<C> {
    pub fn new(
        app_query: c!(C::ApplicationInterface::SyncExecutor),
        notifier: C::NotifierInterface,
        mempool: MempoolSocket,
        signer: TransactionSigner,
        nonce_state: NonceState,
    ) -> Self {
        let chain_id = app_query.get_chain_id();
        Self {
            notifier,
            mempool,
            signer,
            nonce_state,
            chain_id,
        }
    }

    pub async fn execute_transasction(
        &self,
        method: UpdateMethod,
        options: ExecuteTransactionOptions,
    ) -> Result<ExecuteTransactionResponse, ExecuteTransactionError> {
        let mut retry = 0;
        let mut next_nonce = self.nonce_state.get_next_and_increment().await;

        loop {
            // Build and sign the transaction.
            let tx: TransactionRequest = TransactionBuilder::from_update(
                method.clone(),
                self.chain_id,
                next_nonce,
                &self.signer,
            )
            .into();

            // Subscribe to executed blocks notifications before we enqueue the transaction.
            let block_sub = self.notifier.subscribe_block_executed();

            // Send transaction to the mempool.
            match self.mempool.enqueue(tx.clone()).await {
                Ok(()) => {},
                Err(e) => {
                    retry += 1;

                    next_nonce = self
                        .handle_failed_mempool_enqueue(tx, &options, e.to_string(), retry)
                        .await?;

                    continue;
                },
            }

            // Wait for the transaction to be executed and get the receipt.
            let timeout = options.timeout.unwrap_or(DEFAULT_TIMEOUT);
            let result = self
                .wait_for_receipt(method.clone(), tx.clone(), block_sub, timeout)
                .await;
            match result {
                Ok(receipt) => match &receipt.response {
                    TransactionResponse::Success(_) => {
                        tracing::debug!("transaction executed: {:?}", receipt);
                        return Ok(ExecuteTransactionResponse::Receipt((tx, receipt)));
                    },
                    TransactionResponse::Revert(error) => {
                        retry += 1;

                        next_nonce = self
                            .handle_receipt_revert(&tx, &options, &receipt, error, retry)
                            .await?;

                        continue;
                    },
                },
                Err(ExecuteTransactionError::Timeout((method, Some(tx), _))) => {
                    retry += 1;

                    next_nonce = self
                        .handle_receipt_timeout(method, tx, &options, retry)
                        .await?;

                    continue;
                },
                Err(e) => return Err(e),
            }
        }
    }

    async fn handle_failed_mempool_enqueue(
        &self,
        tx: TransactionRequest,
        options: &ExecuteTransactionOptions,
        error: String,
        retry: RetryCount,
    ) -> Result<u64, ExecuteTransactionError> {
        let max_retries = options.retry.get_max_retries(DEFAULT_MAX_RETRIES);

        // Return error if we shouldn't retry.
        if !options.retry.should_retry_on_failure_to_send_to_mempool() {
            tracing::warn!(
                "transaction failed to send to mempool (no retries): {:?}",
                tx
            );
            return Err(ExecuteTransactionError::FailedToSubmitTransactionToMempool(
                (tx, error),
            ));
        }

        // Return error if max retries are reached.
        if retry > max_retries {
            tracing::warn!(
                "transaction failed to send to mempool after max retries reached (attempts: {}): {:?}",
                retry,
                tx
            );
            return Err(ExecuteTransactionError::FailedToSubmitTransactionToMempool(
                (tx, error),
            ));
        }

        // Otherwise, continue to retry
        tracing::warn!(
            "retrying after failing to send transaction to mempool (attempt: {}): {:?}",
            retry,
            tx
        );
        tokio::time::sleep(RETRY_DELAY).await;

        Ok(self.nonce_state.get_next_and_increment().await)
    }

    async fn handle_receipt_revert(
        &self,
        tx: &TransactionRequest,
        options: &ExecuteTransactionOptions,
        receipt: &TransactionReceipt,
        error: &ExecutionError,
        retry: RetryCount,
    ) -> Result<u64, ExecuteTransactionError> {
        let max_retries = options.retry.get_max_retries(DEFAULT_MAX_RETRIES);

        // Return error if we shouldn't retry.
        if !options.retry.should_retry_on_error(error) {
            tracing::warn!("transaction reverted (no retries): {:?}", receipt);
            return Err(ExecuteTransactionError::Reverted((
                tx.clone(),
                receipt.clone(),
                retry,
            )));
        }

        // Return error if max retries are reached.
        if retry > max_retries {
            tracing::warn!(
                "transaction reverted after max retries reached (attempts: {}): {:?}",
                retry,
                receipt
            );
            return Err(ExecuteTransactionError::Reverted((
                tx.clone(),
                receipt.clone(),
                retry,
            )));
        }

        // Otherwise, continue to retry.
        tracing::info!(
            "retrying reverted transaction (hash: {:?}, response: {:?}, attempt: {}): {:?}",
            tx.hash(),
            receipt,
            retry,
            tx
        );
        tokio::time::sleep(RETRY_DELAY).await;

        Ok(self.nonce_state.get_next_and_increment().await)
    }

    async fn handle_receipt_timeout(
        &self,
        method: UpdateMethod,
        tx: TransactionRequest,
        options: &ExecuteTransactionOptions,
        retry: RetryCount,
    ) -> Result<u64, ExecuteTransactionError> {
        let max_retries = options.retry.get_max_retries(DEFAULT_MAX_RETRIES);

        // Return error if we shouldn't retry.
        if !options.retry.should_retry_on_timeout() {
            tracing::warn!("transaction timeout (no retries): {:?}", tx);
            return Err(ExecuteTransactionError::Timeout((method, Some(tx), retry)));
        }

        // Return error if max retries are reached.
        if retry > max_retries {
            tracing::warn!(
                "transaction reverted after max retries reached (attempts: {}): {:?}",
                retry,
                method
            );
            return Err(ExecuteTransactionError::Timeout((method, Some(tx), retry)));
        }

        // Get the transaction nonce.
        let tx_nonce = match &tx {
            TransactionRequest::UpdateRequest(tx) => tx.payload.nonce,
            TransactionRequest::EthereumRequest(tx) => tx.tx.nonce.as_u64(),
        };

        // If the last synced base nonce is less than the transaction nonce, we can retry the
        // transaction with the same nonce.
        if self.nonce_state.get_base().await < tx_nonce {
            tracing::info!(
                "retrying transaction with same nonce (hash: {:?}, attempt: {}): {:?}",
                tx.hash(),
                retry,
                tx
            );
            return Ok(tx_nonce);
        }

        Ok(self.nonce_state.get_next_and_increment().await)
    }

    async fn wait_for_receipt(
        &self,
        method: UpdateMethod,
        tx: TransactionRequest,
        mut block_sub: impl Subscriber<BlockExecutedNotification>,
        timeout: Duration,
    ) -> Result<TransactionReceipt, ExecuteTransactionError> {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(timeout) => {
                    // NOTE: We can use 0 for the number of attempts in the timeout error here because
                    // we will catch this in the caller and inject the actual attempt/retry counter
                    // that's maintained in the main loop.
                    return Err(ExecuteTransactionError::Timeout((method, Some(tx), 0)));
                }
                notification = block_sub.recv() => {
                    let Some(notification) = notification else {
                        tracing::debug!("block subscription stream ended");
                        return Err(ExecuteTransactionError::NotifierShuttingDown);
                    };

                    for receipt in notification.response.txn_receipts {
                        if receipt.transaction_hash == tx.hash() {
                            return Ok(receipt);
                        }
                    }
                }
            }
        }
    }
}
