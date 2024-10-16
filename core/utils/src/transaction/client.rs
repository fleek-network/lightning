use std::time::Duration;

use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    ExecuteTransactionOptions,
    ExecuteTransactionWait,
    UpdateMethod,
};
use tokio::task::JoinHandle;
use types::{ExecuteTransactionError, ExecuteTransactionResponse};

use super::nonce::NonceState;
use super::syncer::TransactionClientNonceSyncer;
use super::TransactionSigner;
use crate::transaction::runner::TransactionRunner;

/// Default max number of times we will resend a transaction.
pub(crate) const DEFAULT_MAX_RETRIES: u8 = 3;

/// Default timeout for waiting for a transaction receipt.
pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

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
    nonce_state: NonceState,
}

impl<C: NodeComponents> TransactionClient<C> {
    pub async fn new(
        app_query: c!(C::ApplicationInterface::SyncExecutor),
        notifier: C::NotifierInterface,
        mempool: MempoolSocket,
        signer: TransactionSigner,
    ) -> Self {
        let nonce_state = NonceState::new(signer.get_nonce(&app_query));

        TransactionClientNonceSyncer::spawn::<C>(
            app_query.clone(),
            notifier.clone(),
            signer.clone(),
            nonce_state.clone(),
        )
        .await;

        Self {
            app_query,
            notifier,
            mempool,
            signer,
            nonce_state,
        }
    }

    pub async fn execute_transaction_and_wait_for_receipt(
        &self,
        method: UpdateMethod,
        options: Option<ExecuteTransactionOptions>,
    ) -> Result<ExecuteTransactionResponse, ExecuteTransactionError> {
        let mut options = options.unwrap_or_default();

        if let ExecuteTransactionWait::None = options.wait {
            options.wait = ExecuteTransactionWait::Receipt;
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
        let options = options.unwrap_or_default();

        // Spawn a tokio task to wait for the transaction receipt, retry if reverted, and return the
        // result containing the transaction request and receipt, or an error.
        let runner_handle = self.spawn_runner(method, options.clone());

        // If we aren't waiting for a receipt, return immediately.
        if let ExecuteTransactionWait::None = options.wait {
            return Ok(ExecuteTransactionResponse::None);
        }

        // Otherwise, wait for the tokio task to complete and return the result.
        let resp = runner_handle.await??;
        Ok(resp)
    }

    fn spawn_runner(
        &self,
        method: UpdateMethod,
        options: ExecuteTransactionOptions,
    ) -> JoinHandle<Result<ExecuteTransactionResponse, ExecuteTransactionError>> {
        let app_query = self.app_query.clone();
        let notifier = self.notifier.clone();
        let mempool = self.mempool.clone();
        let signer = self.signer.clone();
        let nonce_state = self.nonce_state.clone();

        spawn!(
            async move {
                TransactionRunner::<C>::new(app_query, notifier, mempool, signer, nonce_state)
                    .execute_transasction(method, options)
                    .await
            },
            "TRANSACTION-CLIENT: runner"
        )
    }
}
