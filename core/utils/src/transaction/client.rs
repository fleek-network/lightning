use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;

use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    ExecuteTransactionOptions,
    ExecuteTransactionRetry,
    ExecuteTransactionWait,
    UpdateMethod,
};
use types::{ExecuteTransactionError, ExecuteTransactionResponse};

use super::syncer::TransactionClientNonceSyncer;
use super::TransactionSigner;
use crate::transaction::runner::TransactionRunner;

/// Default max number of times we will resend a transaction.
pub(crate) const DEFAULT_MAX_RETRIES: u8 = 3;

/// Default timeout for a transaction to be executed.
pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Default timeout for waiting for a transaction receipt.
pub(crate) const DEFAULT_RECEIPT_TIMEOUT: Duration = Duration::from_secs(20);

/// A client for submitting and executing transactions, and optionally waiting for a receipt, and/or
/// retry if reverted.
///
/// The client is signer-specific, and will sign the incoming transaction with the configured signer
/// before submitting it.
pub struct TransactionClient<C: NodeComponents> {
    app_query: c!(C::ApplicationInterface::SyncExecutor),
    notifier: C::NotifierInterface,
    mempool: MempoolSocket,
    signer: TransactionSigner,
    next_nonce: Arc<AtomicU64>,
}

impl<C: NodeComponents> TransactionClient<C> {
    pub async fn new(
        app_query: c!(C::ApplicationInterface::SyncExecutor),
        notifier: C::NotifierInterface,
        mempool: MempoolSocket,
        signer: TransactionSigner,
    ) -> Self {
        let next_nonce = Arc::new(AtomicU64::new(signer.get_nonce(&app_query) + 1));

        TransactionClientNonceSyncer::spawn::<C>(
            app_query.clone(),
            notifier.clone(),
            signer.clone(),
            next_nonce.clone(),
        )
        .await;

        Self {
            app_query,
            notifier,
            mempool,
            signer,
            next_nonce,
        }
    }

    pub async fn execute_transaction_and_wait_for_receipt(
        &self,
        method: UpdateMethod,
        options: Option<ExecuteTransactionOptions>,
    ) -> Result<ExecuteTransactionResponse, ExecuteTransactionError> {
        let mut options = options.unwrap_or_default();

        if let ExecuteTransactionWait::None = options.wait {
            options.wait = ExecuteTransactionWait::Receipt(None);
        }

        self.execute_transaction(method, Some(options)).await
    }

    /// Submit an update request to the application executor, and optionally wait for a receipt,
    /// and/or retry if reverted.
    ///
    /// If the transaction is not executed within a timeout, an error is returned.
    pub async fn execute_transaction(
        &self,
        method: UpdateMethod,
        options: Option<ExecuteTransactionOptions>,
    ) -> Result<ExecuteTransactionResponse, ExecuteTransactionError> {
        let mut options = options.unwrap_or_default();

        // Default to retrying `MAX_RETRIES` times if not specified.
        match &options.retry {
            ExecuteTransactionRetry::Default => {
                // Default to retrying `DEFAULT_MAX_RETRIES` times for backwards compatibility with
                // signer component expectations.
                options.retry = ExecuteTransactionRetry::Always(Some(DEFAULT_MAX_RETRIES));
            },
            ExecuteTransactionRetry::Always(None) => {
                options.retry = ExecuteTransactionRetry::Always(Some(DEFAULT_MAX_RETRIES));
            },
            ExecuteTransactionRetry::AlwaysExcept((None, errors)) => {
                options.retry = ExecuteTransactionRetry::AlwaysExcept((
                    Some(DEFAULT_MAX_RETRIES),
                    errors.clone(),
                ));
            },
            ExecuteTransactionRetry::OnlyWith((None, errors)) => {
                options.retry =
                    ExecuteTransactionRetry::OnlyWith((Some(DEFAULT_MAX_RETRIES), errors.clone()));
            },
            _ => {},
        }

        // Default timeout to `DEFAULT_TIMEOUT` if not specified.
        if options.timeout.is_none() {
            options.timeout = Some(DEFAULT_TIMEOUT);
        }
        if let ExecuteTransactionWait::Receipt(None) = options.wait {
            options.wait = ExecuteTransactionWait::Receipt(Some(DEFAULT_TIMEOUT));
        }

        // Spawn a tokio task to wait for the transaction receipt, retry if reverted, and return the
        // result containing the transaction request and receipt, or an error.
        let runner_handle = TransactionRunner::<C>::spawn(
            self.app_query.clone(),
            self.notifier.clone(),
            self.mempool.clone(),
            self.signer.clone(),
            self.next_nonce.clone(),
            method,
            options.clone(),
        )
        .await;

        // If we aren't waiting for a receipt, return immediately.
        if let ExecuteTransactionWait::None = options.wait {
            return Ok(ExecuteTransactionResponse::None);
        }

        // Otherwise, wait for the tokio task to complete and return the result.
        let resp = runner_handle.await??;
        Ok(resp)
    }
}
