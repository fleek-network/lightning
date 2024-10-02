use std::time::Duration;

use anyhow::{Context, Result};
use fleek_crypto::{SecretKey, TransactionSender, TransactionSignature};
use lightning_interfaces::prelude::*;
use lightning_utils::application::QueryRunnerExt;
use types::{
    CommitteeSelectionBeaconPhase,
    ExecutionError,
    Metadata,
    NodeIndex,
    TransactionReceipt,
    TransactionRequest,
    TransactionResponse,
    UpdateMethod,
    UpdatePayload,
    UpdateRequest,
    Value,
};

use crate::CommitteeBeaconError;

/// The committee beacon timer is responsible for ensuring that time (blocks) move forward during
/// commit and reveal phases.
///
/// This is done by watching the phase and block number metadata in the application state, and
/// submitting new benign transactions to the mempool to move the phase forward if necessary.
///
/// Most of the time in real world usage, transactions are being submitted by other actors,
/// but this is necessary to ensure that the phase advances if no transactions are submitted.
pub struct CommitteeBeaconTimer<C: NodeComponents> {
    keystore: C::KeystoreInterface,
    notifier: C::NotifierInterface,
    app_query: c!(C::ApplicationInterface::SyncExecutor),
    mempool: MempoolSocket,
    node_index: NodeIndex,
}

impl<C: NodeComponents> CommitteeBeaconTimer<C> {
    pub fn new(
        keystore: C::KeystoreInterface,
        notifier: C::NotifierInterface,
        app_query: c!(C::ApplicationInterface::SyncExecutor),
        mempool: MempoolSocket,
    ) -> Result<Self> {
        let node_secret_key = keystore.get_ed25519_sk();
        let node_public_key = node_secret_key.to_pk();
        let node_index = app_query
            .pubkey_to_index(&node_public_key)
            .context("failed to get node index")?;

        Ok(Self {
            keystore,
            notifier,
            app_query,
            mempool,
            node_index,
        })
    }

    /// Start the committee beacon timer.
    ///
    /// Every tick of the timer, we check if the block number has advanced and that we are in the
    /// commit or reveal phase. If the block number has not advanced, we submit a benign
    /// transaction to move the phase forward.
    pub async fn start(&self) -> Result<(), CommitteeBeaconError> {
        let tick_delay = Duration::from_millis(500);
        let mut prev_block_number = None;
        loop {
            // Get latest block number in application state.
            let block_number = self
                .app_query
                .get_block_number()
                .context("failed to get block number")?;

            // Sleep for the tick duration.
            // TODO(snormore): This tick delay should expontentially back-off if the block number
            // still hasn't changed since last submitting our benign transaction.
            tokio::time::sleep(tick_delay).await;

            // Check if we need to advance the phase based on the block number and phase metadata.
            if let Some(prev) = prev_block_number {
                // Check if the block number hasn't advanced and we're in the commit or reveal
                // phase.
                if block_number == prev && self.in_commit_or_reveal_phase() {
                    tracing::debug!(
                        "block number {} has not advanced since last tick, advancing phase",
                        block_number
                    );

                    // Run a benign transaction to advance the phase.
                    self.execute_transaction(UpdateMethod::IncrementNonce {}, false)
                        .await?;
                }
            }

            // Set our previous block number.
            prev_block_number = Some(block_number);
        }
    }

    fn in_commit_or_reveal_phase(&self) -> bool {
        let phase = self
            .app_query
            .get_metadata(&Metadata::CommitteeSelectionBeaconPhase);
        matches!(
            phase,
            Some(Value::CommitteeSelectionBeaconPhase(
                CommitteeSelectionBeaconPhase::Commit(_,)
            )) | Some(Value::CommitteeSelectionBeaconPhase(
                CommitteeSelectionBeaconPhase::Reveal(_,)
            ))
        )
    }

    // TODO(snormore): DRY this up; same methods in the listener.

    /// Get the node's current nonce.
    fn get_nonce(&self) -> Result<u64, CommitteeBeaconError> {
        self.app_query
            .get_node_info(&self.node_index, |node| node.nonce)
            .ok_or(CommitteeBeaconError::OwnNodeNotFound)
    }

    /// Submit an update request to the application executor and wait for it to be executed. Returns
    /// the transaction request and its receipt.
    ///
    /// If the transaction is not executed within a timeout, an error is returned.
    async fn execute_transaction(
        &self,
        method: UpdateMethod,
        retry_invalid_nonce: bool,
    ) -> Result<(TransactionRequest, Option<TransactionReceipt>), CommitteeBeaconError> {
        let timeout = Duration::from_secs(30);
        let start = tokio::time::Instant::now();
        loop {
            // Get the node's current nonce.
            let nonce = self.get_nonce()?;

            // Build and sign the transaction.
            let tx: TransactionRequest = self.new_update_request(method.clone(), nonce + 1).into();

            // If we've timed out, return an error.
            if start.elapsed() >= timeout {
                return Err(CommitteeBeaconError::TimeoutWaitingForTransactionExecution(
                    tx.hash(),
                ));
            }

            // Send transaction to the mempool.
            // TODO(snormore): We need to subscribe to blocks before sending the transaction or else
            // we may race it and not receive the notification.
            self.mempool
                .run(tx.clone())
                .await
                .map_err(|e| CommitteeBeaconError::SubmitTransaction(e.into()))?;
            // TODO(snormore): Handle connection lost/dropped error here; should we retry or at
            // least return a better error type.

            // Wait for the transaction to be executed, and return the receipt.
            let receipt = self.wait_for_receipt(tx.clone()).await?;

            // Retry if the transaction was reverted because of invalid nonce.
            // This means our node sent multiple transactions asynchronously that landed in the same
            // block.
            if retry_invalid_nonce {
                if let Some(receipt) = &receipt {
                    if receipt.response == TransactionResponse::Revert(ExecutionError::InvalidNonce)
                    {
                        tracing::warn!(
                            "transaction {:?} reverted due to invalid nonce, retrying",
                            tx.hash()
                        );
                        continue;
                    }
                }
            }

            return Ok((tx, receipt));
        }
    }

    /// Wait for a transaction receipt for a given transaction.
    ///
    /// If the transaction is not executed within a timeout, an error is returned.
    async fn wait_for_receipt(
        &self,
        tx: TransactionRequest,
    ) -> Result<Option<TransactionReceipt>, CommitteeBeaconError> {
        // TODO(snormore): Consider using a shared subscription.
        let mut block_sub = self.notifier.subscribe_block_executed();
        // TODO(snormore): What should this timeout be?
        let timeout_fut = tokio::time::sleep(Duration::from_secs(30));
        tokio::pin!(timeout_fut);
        loop {
            tokio::select! {
                notification = block_sub.recv() => {
                    match notification {
                        Some(notification) => {
                            let response = notification.response;
                            for receipt in response.txn_receipts {
                                if receipt.transaction_hash == tx.hash() {
                                    match receipt.response {
                                        TransactionResponse::Success(_) => {
                                            tracing::debug!("transaction executed: {:?}", receipt);
                                        },
                                        TransactionResponse::Revert(_) => {
                                            // TODO(snormore): What to do here or in the caller/listener when transactions are reverted?
                                            tracing::warn!("transaction reverted: {:?}", receipt);
                                        },
                                    }
                                    return Ok(Some(receipt));
                                }
                            }
                            continue;
                        },
                        None => {
                            // Notifier is not running, exit
                            // TODO(snormore): Should we return an error here?
                            return Ok(None);
                        }
                    }
                },
                _ = &mut timeout_fut => {
                    tracing::warn!("timeout while waiting for transaction receipt: {:?}", tx.hash());
                    return Err(
                        CommitteeBeaconError::TimeoutWaitingForTransactionReceipt(tx.hash()),
                    );
                },
            }
        }
    }

    /// Build and sign a new update request.
    fn new_update_request(&self, method: UpdateMethod, nonce: u64) -> UpdateRequest {
        let chain_id = self.app_query.get_chain_id();
        let node_secret_key = self.keystore.get_ed25519_sk();
        let node_public_key = node_secret_key.to_pk();
        let payload = UpdatePayload {
            sender: TransactionSender::NodeMain(node_public_key),
            nonce,
            method,
            chain_id,
        };
        let digest = payload.to_digest();
        let signature = node_secret_key.sign(&digest);

        UpdateRequest {
            payload,
            signature: TransactionSignature::NodeMain(signature),
        }
    }
}
