use std::time::Duration;

use affair::RunError;
use lightning_interfaces::prelude::*;
use lightning_interfaces::BlockExecutedNotification;
use types::{
    ChainId,
    ExecuteTransactionError,
    ExecuteTransactionOptions,
    ExecuteTransactionResponse,
    ExecutionError,
    ForwarderError,
    Nonce,
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
            match self.mempool.run(tx.clone()).await {
                Ok(result) => match result {
                    Ok(()) => {},
                    Err(error) => {
                        retry += 1;
                        next_nonce = self
                            .handle_forwarder_error(tx, &options, error, retry)
                            .await?;
                        continue;
                    },
                },
                Err(error) => {
                    retry += 1;
                    next_nonce = self
                        .handle_forwarder_run_error(tx, &options, error, retry)
                        .await?;
                    continue;
                },
            }

            // Wait for the transaction to be executed and get the receipt.
            let timeout = options.timeout.unwrap_or(DEFAULT_TIMEOUT);
            let result = self
                .wait_for_receipt(method.clone(), &tx, block_sub, timeout)
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
                            .handle_receipt_revert(&tx, &options, &receipt, error, retry, timeout)
                            .await?;
                        continue;
                    },
                },
                Err(ExecuteTransactionError::Timeout((method, Some(tx), _))) => {
                    retry += 1;
                    next_nonce = self
                        .handle_receipt_timeout(method, &tx, &options, retry, timeout)
                        .await?;
                    continue;
                },
                Err(e) => return Err(e),
            }
        }
    }

    async fn handle_forwarder_run_error(
        &self,
        tx: TransactionRequest,
        options: &ExecuteTransactionOptions,
        error: RunError<TransactionRequest>,
        retry: RetryCount,
    ) -> Result<Nonce, ExecuteTransactionError> {
        // Return error if we shouldn't retry.
        let Some((max_retries, delay)) = options.retry.should_retry_on_forwarder_run_error(&error)
        else {
            tracing::warn!(
                "transaction failed to send to mempool (no retries): {:?}",
                tx
            );
            return Err(ExecuteTransactionError::ForwarderRunError((tx, error)));
        };

        // Return error if max retries are reached.
        if retry > max_retries {
            tracing::warn!(
                "transaction failed to send to mempool after max retries reached (attempts: {}): {:?}",
                retry,
                tx
            );
            return Err(ExecuteTransactionError::ForwarderRunError((tx, error)));
        }

        // Otherwise, continue to retry
        tracing::warn!(
            "retrying after failing to send transaction to mempool (attempt: {}): {:?}",
            retry,
            tx
        );
        tokio::time::sleep(delay).await;

        // Retry with same nonce.
        Ok(tx.nonce())
    }

    async fn handle_forwarder_error(
        &self,
        tx: TransactionRequest,
        options: &ExecuteTransactionOptions,
        error: ForwarderError,
        retry: RetryCount,
    ) -> Result<Nonce, ExecuteTransactionError> {
        // Return error if we shouldn't retry.
        let Some((max_retries, delay)) = options.retry.should_retry_on_forwarder_error(&error)
        else {
            tracing::warn!(
                "transaction failed to send to mempool (no retries): {:?}",
                tx
            );
            return Err(ExecuteTransactionError::ForwarderError((tx, error)));
        };

        // Return error if max retries are reached.
        if retry > max_retries {
            tracing::warn!(
                "transaction failed to send to mempool after max retries reached (attempts: {}): {:?}",
                retry,
                tx
            );
            return Err(ExecuteTransactionError::ForwarderError((tx, error)));
        }

        // Otherwise, continue to retry.
        tracing::warn!(
            "retrying after failing to send transaction to mempool (attempt: {}): {:?}",
            retry,
            tx
        );
        tokio::time::sleep(delay).await;

        // Retry with same nonce.
        Ok(tx.nonce())
    }

    async fn handle_receipt_revert(
        &self,
        tx: &TransactionRequest,
        options: &ExecuteTransactionOptions,
        receipt: &TransactionReceipt,
        error: &ExecutionError,
        retry: RetryCount,
        timeout: Duration,
    ) -> Result<Nonce, ExecuteTransactionError> {
        // Return error if we shouldn't retry.
        let Some((max_retries, delay)) = options.retry.should_retry_on_revert(error) else {
            tracing::debug!("transaction reverted (no retries): {:?}", receipt);
            self.ensure_nonce_used_before_giving_up(tx, options, timeout)
                .await?;
            return Err(ExecuteTransactionError::Reverted((
                tx.clone(),
                receipt.clone(),
                retry,
            )));
        };

        // Return error if max retries are reached.
        if retry > max_retries {
            tracing::warn!(
                "transaction reverted after max retries reached (attempts: {}): {:?}",
                retry,
                receipt
            );
            self.ensure_nonce_used_before_giving_up(tx, options, timeout)
                .await?;
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
        tokio::time::sleep(delay).await;

        // Retry with next available nonce.
        Ok(self.nonce_state.get_next_and_increment().await)
    }

    async fn handle_receipt_timeout(
        &self,
        method: UpdateMethod,
        tx: &TransactionRequest,
        options: &ExecuteTransactionOptions,
        retry: RetryCount,
        timeout: Duration,
    ) -> Result<Nonce, ExecuteTransactionError> {
        let max_retries = options.retry.get_max_retries(DEFAULT_MAX_RETRIES);

        // Return error if we shouldn't retry.
        if !options.retry.should_retry_on_timeout() {
            tracing::warn!("transaction timeout (no retries): {:?}", tx);
            self.ensure_nonce_used_before_giving_up(tx, options, timeout)
                .await?;
            return Err(ExecuteTransactionError::Timeout((
                method,
                Some(tx.clone()),
                retry,
            )));
        }

        // Return error if max retries are reached.
        if retry > max_retries {
            tracing::warn!(
                "transaction reverted after max retries reached (attempts: {}): {:?}",
                retry,
                method
            );
            self.ensure_nonce_used_before_giving_up(tx, options, timeout)
                .await?;
            return Err(ExecuteTransactionError::Timeout((
                method,
                Some(tx.clone()),
                retry,
            )));
        }

        // Otherwise, we can retry again, but we need to backfill the nonce first to avoid
        // submitting the same transaction more than once with different nonces.
        self.ensure_nonce_used(tx, options, timeout).await?;
        tracing::warn!(
            "retrying transaction after timeout (hash: {:?}, attempt: {}): {:?}",
            tx.hash(),
            retry,
            tx
        );

        // Retry with next available nonce.
        Ok(self.nonce_state.get_next_and_increment().await)
    }

    async fn wait_for_receipt(
        &self,
        method: UpdateMethod,
        tx: &TransactionRequest,
        mut block_sub: impl Subscriber<BlockExecutedNotification>,
        timeout: Duration,
    ) -> Result<TransactionReceipt, ExecuteTransactionError> {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(timeout) => {
                    // NOTE: We can use 0 for the number of attempts in the timeout error here because
                    // we will catch this in the caller and inject the actual attempt/retry counter
                    // that's maintained in the main loop.
                    return Err(ExecuteTransactionError::Timeout((method, Some(tx.clone()), 0)));
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

    async fn ensure_nonce_used_before_giving_up(
        &self,
        tx: &TransactionRequest,
        options: &ExecuteTransactionOptions,
        timeout: Duration,
    ) -> Result<(), ExecuteTransactionError> {
        let tx_nonce = match tx {
            TransactionRequest::UpdateRequest(tx) => tx.payload.nonce,
            TransactionRequest::EthereumRequest(tx) => tx.tx.nonce.as_u64(),
        };

        // If there are no pending transactions in front of this one, then we don't need to backfill
        // the nonce before giving up.
        if self.nonce_state.get_next().await <= tx_nonce + 1 {
            return Ok(());
        }

        self.ensure_nonce_used(tx, options, timeout).await?;

        Ok(())
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

            // If the last known used nonce is this transaction nonce or later, then we don't need
            // to backfill the nonce.
            if tx_nonce <= last_used_nonce {
                return Ok(());
            }

            tracing::warn!(
                "previous transaction nonce {} not used, backfilling it with an IncrementNonce transaction",
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
                        // We give up trying to backfill the nonce here, but we don't want to error
                        // out of this, so we log and continue.
                        return Ok(());
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
            // If the original transaction hash matches the IncrementNonce transaction hash, then
            // the original was an IncrementNonce transaction, and so we don't want to error out
            // here.
            if tx.hash() != increment_nonce_txn.hash()
                && self.app_query.has_executed_digest(tx.hash())
            {
                // If the original transaction was executed, we're done, no need to retry.
                // Unfortunately, we can't return a receipt here without using an archive node
                // to get historic transactions data because we stopped listening for the
                // receipt after timeout.
                return Err(
                    ExecuteTransactionError::TransactionExecutedButReceiptNotAvailable(tx.clone()),
                );
            }

            // Otherwise, we're done.
            return Ok(());
        }
    }
}
