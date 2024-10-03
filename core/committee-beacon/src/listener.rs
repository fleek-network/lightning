use anyhow::{Context, Result};
use fleek_crypto::SecretKey;
use lightning_interfaces::prelude::*;
use lightning_utils::transaction::{TransactionClient, TransactionSigner};
use rand::Rng;
use sha3::{Digest, Sha3_256};
use types::{
    BlockExecutionResponse,
    BlockNumber,
    CommitteeSelectionBeaconPhase,
    CommitteeSelectionBeaconReveal,
    Metadata,
    NodeIndex,
    UpdateMethod,
    Value,
};

use crate::database::{CommitteeBeaconDatabase, CommitteeBeaconDatabaseQuery};
use crate::rocks::RocksCommitteeBeaconDatabase;
use crate::CommitteeBeaconError;

pub struct CommitteeBeaconListener<C: NodeComponents> {
    db: RocksCommitteeBeaconDatabase,
    notifier: C::NotifierInterface,
    app_query: c!(C::ApplicationInterface::SyncExecutor),
    node_index: NodeIndex,
    client: TransactionClient<C>,
}

impl<C: NodeComponents> CommitteeBeaconListener<C> {
    pub async fn new(
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

        // Build a new transaction client for executing transactions signed by this node.
        let client = TransactionClient::new(
            app_query.clone(),
            notifier.clone(),
            mempool.clone(),
            TransactionSigner::NodeMain(node_secret_key),
        )
        .await;

        Ok(Self {
            db,
            notifier,
            app_query,
            node_index,
            client,
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
            self.client
                .execute_transaction_with_retry_on_invalid_nonce(
                    UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout,
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
        self.client
            .execute_transaction_with_retry_on_invalid_nonce(
                UpdateMethod::CommitteeSelectionBeaconCommit { commit },
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
            self.client
                .execute_transaction_with_retry_on_invalid_nonce(
                    UpdateMethod::CommitteeSelectionBeaconRevealPhaseTimeout,
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
        self.client
            .execute_transaction_with_retry_on_invalid_nonce(
                UpdateMethod::CommitteeSelectionBeaconReveal { reveal },
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
}
