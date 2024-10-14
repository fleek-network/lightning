use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use lightning_interfaces::prelude::*;
use lightning_interfaces::BlockExecutedNotification;
use tokio::task::JoinHandle;
use types::{
    ExecuteTransactionError,
    ExecuteTransactionOptions,
    ExecuteTransactionResponse,
    ExecuteTransactionRetry,
    ExecuteTransactionWait,
    TransactionReceipt,
    TransactionRequest,
    TransactionResponse,
    UpdateMethod,
};

use super::{TransactionBuilder, TransactionSigner, DEFAULT_RECEIPT_TIMEOUT};
use crate::application::QueryRunnerExt;

pub struct TransactionRunner<C: NodeComponents> {
    app_query: c!(C::ApplicationInterface::SyncExecutor),
    notifier: C::NotifierInterface,
    mempool: MempoolSocket,
    signer: TransactionSigner,
    next_nonce: Arc<AtomicU64>,
}

impl<C: NodeComponents> TransactionRunner<C> {
    pub fn new(
        app_query: c!(C::ApplicationInterface::SyncExecutor),
        notifier: C::NotifierInterface,
        mempool: MempoolSocket,
        signer: TransactionSigner,
        next_nonce: Arc<AtomicU64>,
    ) -> Self {
        Self {
            app_query,
            notifier,
            mempool,
            signer,
            next_nonce,
        }
    }

    pub async fn spawn(
        app_query: c!(C::ApplicationInterface::SyncExecutor),
        notifier: C::NotifierInterface,
        mempool: MempoolSocket,
        signer: TransactionSigner,
        next_nonce: Arc<AtomicU64>,
        method: UpdateMethod,
        options: ExecuteTransactionOptions,
    ) -> JoinHandle<Result<ExecuteTransactionResponse, ExecuteTransactionError>> {
        let runner = Self::new(app_query, notifier, mempool, signer, next_nonce);
        spawn!(
            async move { runner.execute(method, options).await },
            "TRANSACTION-CLIENT: runner"
        )
    }

    pub async fn execute(
        self,
        method: UpdateMethod,
        options: ExecuteTransactionOptions,
    ) -> Result<ExecuteTransactionResponse, ExecuteTransactionError> {
        let chain_id = self.app_query.get_chain_id();
        let mut retry = 0;

        let receipt_timeout = match options.wait {
            ExecuteTransactionWait::Receipt(timeout) => timeout.unwrap_or(DEFAULT_RECEIPT_TIMEOUT),
            ExecuteTransactionWait::None => DEFAULT_RECEIPT_TIMEOUT,
        };

        loop {
            // Get the next nonce for this transaction.
            let next_nonce = self.next_nonce.fetch_add(1, Ordering::SeqCst);

            // Build and sign the transaction.
            let tx: TransactionRequest =
                TransactionBuilder::from_update(method.clone(), chain_id, next_nonce, &self.signer)
                    .into();

            // Subscribe to executed blocks notifications before we enqueue the transaction.
            let block_sub = self.notifier.subscribe_block_executed();

            // Send transaction to the mempool.
            self.send_to_mempool(tx.clone()).await?;

            // Wait for the transaction to be executed and get the receipt.
            let receipt = match self
                .wait_for_receipt(method.clone(), tx.clone(), block_sub, receipt_timeout)
                .await
            {
                Ok(receipt) => receipt,
                Err(ExecuteTransactionError::Timeout((method, Some(tx), _))) => {
                    // Increment the retry counter.
                    retry += 1;

                    // If we have a max retries limit, check if we've hit it.
                    let (should_retry, max_retries) = match options.retry {
                        ExecuteTransactionRetry::OnlyWith((max_retries, _, retry_on_timeout)) => {
                            (retry_on_timeout, max_retries)
                        },
                        ExecuteTransactionRetry::AlwaysExcept((
                            max_retries,
                            _,
                            retry_on_timeout,
                        )) => (retry_on_timeout, max_retries),
                        ExecuteTransactionRetry::Always(max_retries) => (true, max_retries),
                        ExecuteTransactionRetry::Never => (false, None),
                        ExecuteTransactionRetry::Default => unreachable!(),
                    };

                    // If we shouldn't retry, return the timeout error.
                    if !should_retry {
                        return Err(ExecuteTransactionError::Timeout((method, Some(tx), retry)));
                    }

                    // If we have a max retries limit, check if we've hit it.
                    if let Some(max_retries) = max_retries {
                        if retry > max_retries {
                            tracing::warn!(
                                "transaction reverted after max retries reached (attempts: {}): {:?}",
                                retry,
                                method
                            );
                            return Err(ExecuteTransactionError::Timeout((
                                method,
                                Some(tx),
                                retry,
                            )));
                        }
                    }

                    tracing::warn!(
                        "retrying transaction after timeout (hash: {:?}, attempt: {}): {:?}",
                        tx.hash(),
                        retry + 1,
                        tx
                    );

                    // Continue to retry.
                    continue;
                },
                Err(e) => return Err(e),
            };

            // Determine if we should retry the transaction based on the receipt.
            let (should_retry, max_retries) = match &receipt.response {
                // If the transaction was successful, return the receipt.
                TransactionResponse::Success(_) => {
                    tracing::debug!("transaction executed: {:?}", receipt);
                    return Ok(ExecuteTransactionResponse::Receipt((tx, receipt)));
                },

                // If the transaction reverted, retry or return an error depending on the retry
                // configuration and if we are within the limit.
                TransactionResponse::Revert(error) => match options.retry {
                    ExecuteTransactionRetry::OnlyWith((
                        max_retries,
                        ref errors,
                        _retry_on_timeout,
                    )) => (
                        errors
                            .as_ref()
                            .map_or(false, |errors| errors.contains(error)),
                        max_retries,
                    ),
                    ExecuteTransactionRetry::AlwaysExcept((
                        max_retries,
                        ref errors,
                        _retry_on_timeout,
                    )) => (
                        errors
                            .as_ref()
                            .map_or(true, |errors| !errors.contains(error)),
                        max_retries,
                    ),
                    ExecuteTransactionRetry::Always(max_retries) => (true, max_retries),
                    ExecuteTransactionRetry::Never => (false, None),
                    ExecuteTransactionRetry::Default => {
                        unreachable!("default should be handled in caller")
                    },
                },
            };

            // Increment the retry counter.
            retry += 1;

            // If we shouldn't retry, return the receipt.
            if !should_retry {
                tracing::warn!("transaction reverted (no retries): {:?}", receipt);
                return Err(ExecuteTransactionError::Reverted((tx, receipt, retry)));
            }

            // If we have a max retries limit, check if we've hit it.
            if let Some(max_retries) = max_retries {
                if retry > max_retries {
                    tracing::warn!(
                        "transaction reverted after max retries reached (attempts: {}): {:?}",
                        retry,
                        receipt
                    );
                    return Err(ExecuteTransactionError::Reverted((tx, receipt, retry)));
                }
            }

            // Otherwise, continue to retry.
            tracing::info!(
                "retrying reverted transaction (hash: {:?}, response: {:?}, attempt: {}): {:?}",
                tx.hash(),
                receipt,
                retry + 1,
                tx
            );

            // Sleep for a short duration before retrying.
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn send_to_mempool(&self, tx: TransactionRequest) -> Result<(), ExecuteTransactionError> {
        self.mempool.enqueue(tx.clone()).await.map_err(|e| {
            ExecuteTransactionError::FailedToSubmitTransactionToMempool((tx.clone(), e.to_string()))
        })
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
