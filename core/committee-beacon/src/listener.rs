use std::time::Duration;

use anyhow::{Context, Result};
use fleek_crypto::{SecretKey, TransactionSender, TransactionSignature};
use lightning_interfaces::prelude::*;
use lightning_utils::application::QueryRunnerExt;
use rand::Rng;
use sha3::{Digest, Sha3_256};
use types::{
    BlockExecutionResponse,
    BlockNumber,
    CommitteeSelectionBeaconPhase,
    CommitteeSelectionBeaconReveal,
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

use crate::database::{CommitteeBeaconDatabase, CommitteeBeaconDatabaseQuery};
use crate::rocks::RocksCommitteeBeaconDatabase;
use crate::CommitteeBeaconError;

pub struct CommitteeBeaconListener<C: NodeComponents> {
    db: RocksCommitteeBeaconDatabase,
    keystore: C::KeystoreInterface,
    notifier: C::NotifierInterface,
    app_query: c!(C::ApplicationInterface::SyncExecutor),
    node_index: NodeIndex,
    mempool: MempoolSocket,
}

impl<C: NodeComponents> CommitteeBeaconListener<C> {
    pub fn new(
        db: RocksCommitteeBeaconDatabase,
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
            db,
            notifier,
            keystore,
            app_query,
            node_index,
            mempool,
        })
    }

    /// Start the committee beacon listener.
    ///
    /// This method listens for execution notifications and contributes to the commit and reveal
    /// phases of the process.
    pub async fn start(&self) -> Result<(), CommitteeBeaconError> {
        tracing::debug!("starting committee beacon listener");

        // Subscribe to notifications for executed blocks.
        let mut block_sub = self.notifier.subscribe_block_executed();
        loop {
            let notification = block_sub.recv().await;

            // Check that the notifier is still running.
            if notification.is_none() {
                tracing::debug!("notifier is not running, shutting down");
                break;
            }
            let response = notification.unwrap().response;

            // Handle the executed block.
            self.handle_executed_block(response).await?;
        }

        tracing::debug!("shutdown committee beacon listener");
        Ok(())
    }

    async fn handle_executed_block(
        &self,
        response: BlockExecutionResponse,
    ) -> Result<(), CommitteeBeaconError> {
        tracing::trace!("handling block execution response: {:?}", response);

        // Clear the local database after epoch change is executed.
        if response.change_epoch {
            tracing::debug!("clearing beacons at epoch change");
            // Clearing the beacons at epoch change is best-effort, since we can't guarantee that
            // the notification will be received or the listener will be running, in the case of a
            // deployment for example. This is fine, since the beacons will be cleared on the next
            // committee selection phase anyway, and we don't rely on it for correctness.
            self.db.clear_beacons();
            return Ok(());
        }

        // Get the current phase from metadata.
        let phase = self
            .app_query
            .get_metadata(&Metadata::CommitteeSelectionBeaconPhase);

        // Handle the current phase.
        match phase {
            None => {
                // If the phase is not set, do nothing.
                return Ok(());
            },
            Some(Value::CommitteeSelectionBeaconPhase(CommitteeSelectionBeaconPhase::Commit(
                (start_block, end_block),
            ))) => {
                let result = self
                    .handle_commit_phase(start_block, end_block, response.clone())
                    .await;

                // Handling executed block responses is best-effort, and so if the commit phase
                // handler fails for this block notification, we should log an error and continue,
                // rather than returning an error and stopping the component.
                // TODO(snormore): Can we write a test for this?
                if let Err(e) = result {
                    tracing::error!("failed to handle commit phase: {:?}", e);
                }
            },
            Some(Value::CommitteeSelectionBeaconPhase(CommitteeSelectionBeaconPhase::Reveal(
                (start_block, end_block),
            ))) => {
                let result = self
                    .handle_reveal_phase(start_block, end_block, response.clone())
                    .await;

                // Handling executed block responses is best-effort, and so if the reveal phase
                // handler fails for this block notification, we should log an error and continue,
                // rather than returning an error and stopping the component.
                // TODO(snormore): Can we write a test for this?
                if let Err(e) = result {
                    tracing::error!("failed to handle reveal phase: {:?}", e);
                }
            },
            _ => {
                // This should never happen, but if it does, we error out.
                return Err(CommitteeBeaconError::UnknownPhaseType(phase));
            },
        };

        Ok(())
    }

    /// Handle a block execution response that is in the commit phase.
    ///
    /// If the node has not committed yet, it will send a commit transaction. If the node has
    /// committed, it will do nothing.
    ///
    /// If this is the last block of the commit phase, it will trigger an update in the
    /// application executor that will move onto the reveal phase, or restart the
    /// commit phase, depending on participation.
    async fn handle_commit_phase(
        &self,
        start_block: BlockNumber,
        end_block: BlockNumber,
        response: BlockExecutionResponse,
    ) -> Result<(), CommitteeBeaconError> {
        tracing::debug!(
            "handling commit phase ({}, {}) at block {}",
            start_block,
            end_block,
            response.block_number
        );

        // If we are not in the commit phase block range, do nothing.
        if response.block_number < start_block || response.block_number > end_block {
            return Ok(());
        }

        // If this was the last block of the commit phase, or we are outside of the range, trigger
        // an update in the application executor that will move onto the reveal phase, or
        // restart the commit phase, depending on participation.
        if response.block_number >= end_block {
            tracing::debug!("submitting commit phase timeout transaction");
            self.execute_transaction(
                UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout,
                true,
            )
            .await?;

            // Return, we don't need to do anything else.
            return Ok(());
        }

        // If we have already committed, there's nothing to do.
        let node_beacon = self
            .app_query
            .get_committee_selection_beacon(&self.node_index);
        if node_beacon.is_some() {
            tracing::debug!("node {} already committed in commit phase", self.node_index);
            return Ok(());
        }

        // If we have not committed, generate a random value for our commit transaction, save to
        // local database, and submit it.
        // Generate random beacon data.
        let reveal = self.generate_random_reveal();
        let commit = Sha3_256::digest(reveal).into();

        // Save random beacon data to local database.
        self.db.set_beacon(commit, reveal);

        // Build commit and submit it.
        tracing::debug!("submitting commit transaction: {:?}", commit);
        self.execute_transaction(
            UpdateMethod::CommitteeSelectionBeaconCommit { commit },
            true,
        )
        .await?;

        Ok(())
    }

    /// Handle a block execution response that is in the reveal phase.
    ///
    /// This method is called when a block execution response is received and the reveal phase is
    /// active. If the node has not revealed yet, it will send a reveal transaction. If the node
    /// has revealed, it will do nothing.
    ///
    /// If this is the last block of the reveal phase, it will trigger an update in the
    /// application executor that will restart back to a new commit phase.
    async fn handle_reveal_phase(
        &self,
        start_block: BlockNumber,
        end_block: BlockNumber,
        response: BlockExecutionResponse,
    ) -> Result<(), CommitteeBeaconError> {
        tracing::debug!(
            "handling reveal phase ({}, {}) at block {}",
            start_block,
            end_block,
            response.block_number
        );

        // If we are not in the reveal phase block range, do nothing.
        if response.block_number < start_block {
            return Ok(());
        }

        // If this was the last block of the reveal phase, or we are outside of the range, trigger
        // an update in the application executor that will restart back to a new commit
        // phase.
        if response.block_number >= end_block {
            tracing::debug!("submitting reveal phase timeout transaction");
            self.execute_transaction(
                UpdateMethod::CommitteeSelectionBeaconRevealPhaseTimeout,
                true,
            )
            .await?;

            // Return, we don't need to do anything else.
            return Ok(());
        }

        // If we have not committed, there's nothing to reveal.
        let node_beacon = self
            .app_query
            .get_committee_selection_beacon(&self.node_index);
        if node_beacon.is_none() {
            tracing::debug!("node {} has not committed in reveal phase", self.node_index);
            return Ok(());
        }

        // If we have already revealed, there's nothing to do.
        let (commit, reveal) = node_beacon.unwrap();
        if reveal.is_some() {
            tracing::debug!("node {} already revealed in reveal phase", self.node_index);
            return Ok(());
        }

        // Retrieve our beacon random data from local database.
        let reveal = self.db.query().get_beacon(commit);
        if reveal.is_none() {
            tracing::warn!("no beacon found for reveal in local database");
            return Ok(());
        }
        let reveal = reveal.unwrap();

        // Otherwise, send our reveal transaction.
        tracing::debug!("submitting reveal transaction: {:?}", reveal);
        self.execute_transaction(
            UpdateMethod::CommitteeSelectionBeaconReveal { reveal },
            true,
        )
        .await?;

        Ok(())
    }

    /// Generate random reveal value.
    fn generate_random_reveal(&self) -> CommitteeSelectionBeaconReveal {
        let mut rng = rand::thread_rng();
        let mut reveal = [0u8; 32];
        rng.fill(&mut reveal);
        reveal
    }

    // TODO(snormore): DRY this up; same methods in the timer.

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
