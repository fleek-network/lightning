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
use crate::poll::{poll_until, PollUntilError};

const RETRY_DELAY: Duration = Duration::from_millis(100);

type RetryCount = u8;

pub struct TransactionRunner<C: NodeComponents> {
    app_query: c!(C::ApplicationInterface::SyncExecutor),
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
            app_query,
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

        loop {
            // Get the next nonce for this transaction.
            let next_nonce = self.nonce_state.get_next_and_increment().await;

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
                    self.handle_failed_mempool_enqueue(tx, &options, e.to_string(), retry)
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
                        self.handle_receipt_revert(&tx, &options, &receipt, error, retry)
                            .await?;
                        continue;
                    },
                },
                Err(ExecuteTransactionError::Timeout((method, Some(tx), _))) => {
                    retry += 1;
                    self.handle_receipt_timeout(method, tx, &options, retry, timeout)
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
    ) -> Result<(), ExecuteTransactionError> {
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

        Ok(())
    }

    async fn handle_receipt_revert(
        &self,
        tx: &TransactionRequest,
        options: &ExecuteTransactionOptions,
        receipt: &TransactionReceipt,
        error: &ExecutionError,
        retry: RetryCount,
    ) -> Result<(), ExecuteTransactionError> {
        let max_retries = options.retry.get_max_retries(DEFAULT_MAX_RETRIES);

        // Return error if we shouldn't retry.
        if !options.retry.should_retry_on_error(error) {
            tracing::debug!("transaction reverted (no retries): {:?}", receipt);
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

        Ok(())
    }

    async fn handle_receipt_timeout(
        &self,
        method: UpdateMethod,
        tx: TransactionRequest,
        options: &ExecuteTransactionOptions,
        retry: RetryCount,
        timeout: Duration,
    ) -> Result<RetryCount, ExecuteTransactionError> {
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

        // Ensure the previous transaction nonce has been used before retrying to
        // avoid executing the same transaction twice.
        match self.ensure_nonce_used(&tx, options, timeout).await {
            Ok(()) => {
                // Otherwise, continue to retry.
                tracing::warn!(
                    "retrying transaction after timeout (hash: {:?}, attempt: {}): {:?}",
                    tx.hash(),
                    retry,
                    tx
                );
                Ok(retry)
            },
            Err(ExecuteTransactionError::FailedToIncrementNonceForRetry(_)) => {
                // If the IncrementNonce transaction fails to execute and we still have
                // some retries left, then we can continue on to retry again until we've
                // exhausted the retries.
                tracing::warn!("retrying after failing to execute IncrementNonce: {:?}", tx);
                tokio::time::sleep(RETRY_DELAY).await;
                Ok(retry)
            },
            Err(e) => Err(e),
        }
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

    async fn ensure_nonce_used(
        &self,
        tx: &TransactionRequest,
        options: &ExecuteTransactionOptions,
        timeout: Duration,
    ) -> Result<(), ExecuteTransactionError> {
        let tx_nonce = match tx {
            TransactionRequest::UpdateRequest(tx) => tx.payload.nonce,
            TransactionRequest::EthereumRequest(tx) => tx.tx.nonce.as_u64(),
        };
        let mut retry = 0;
        let max_retries = options.retry.get_max_retries(DEFAULT_MAX_RETRIES);
        loop {
            let last_used_nonce = self.nonce_state.get_base().await;
            if tx_nonce > last_used_nonce {
                tracing::warn!(
                    "previous transaction nonce {} not used, submitting an IncrementNonce in place before retrying",
                    tx_nonce
                );

                // Build the IncrementNonce transaction.
                let increment_nonce_txn: TransactionRequest = TransactionBuilder::from_update(
                    UpdateMethod::IncrementNonce {},
                    self.chain_id,
                    tx_nonce,
                    &self.signer,
                )
                .into();

                // Send it to the mempool.
                match self.mempool.enqueue(increment_nonce_txn.clone()).await {
                    Ok(()) => {},
                    Err(e) => {
                        retry += 1;
                        if retry > max_retries {
                            tracing::warn!(
                                "failed to enqueue IncrementNonce transaction after max retries (attempts: {}): {:?}",
                                retry,
                                e
                            );
                            return Err(ExecuteTransactionError::FailedToIncrementNonceForRetry((
                                increment_nonce_txn,
                                e.to_string(),
                            )));
                        }
                        tracing::warn!(
                            "retrying after failing to enqueue IncrementNonce (attempt {}): {:?}",
                            retry,
                            e
                        );
                        tokio::time::sleep(RETRY_DELAY).await;
                        continue;
                    },
                }

                // Wait for the nonce to be incremented.
                let result = poll_until(
                    || async {
                        let last_used_nonce = self.nonce_state.get_base().await;
                        (last_used_nonce >= tx_nonce)
                            .then_some(())
                            .ok_or(PollUntilError::ConditionNotSatisfied)
                    },
                    timeout,
                    Duration::from_millis(100),
                )
                .await;
                match result {
                    Ok(()) => {},
                    Err(e) => {
                        retry += 1;
                        if retry > max_retries {
                            tracing::warn!(
                                "failed to increment nonce after max retries (attempts: {}): {:?}",
                                retry,
                                e
                            );
                            // At this point it looks like there's a liveness issue with blocks not
                            // moving forward or the node is in bad shape and is losing submitted
                            // transactions.
                            return Err(ExecuteTransactionError::FailedToIncrementNonceForRetry((
                                increment_nonce_txn,
                                e.to_string(),
                            )));
                        }
                        tracing::warn!(
                            "retrying after failing to increment nonce (attempt {}): {:?}",
                            retry,
                            e
                        );
                        tokio::time::sleep(RETRY_DELAY).await;
                        continue;
                    },
                }

                // Make sure that the original transaction was not executed in the meantime.
                // This should be a very rare edge case.
                if self.app_query.has_executed_digest(tx.hash()) {
                    // If the original transaction was executed, we're done, no need to retry.
                    // Unfortunately, we can't return a receipt here without using an archive node
                    // to get historic transactions data because we stopped listening for the
                    // receipt after timeout.
                    return Err(
                        ExecuteTransactionError::TransactionExecutedButReceiptNotAvailable(
                            tx.clone(),
                        ),
                    );
                }

                // Otherwise, we're done.
                return Ok(());
            }
        }
    }
}
