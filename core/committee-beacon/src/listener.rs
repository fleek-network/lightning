use anyhow::Result;
use lightning_interfaces::prelude::*;
use lightning_utils::application::QueryRunnerExt;
use rand::Rng;
use types::{
    BlockExecutionResponse,
    BlockNumber,
    CommitteeSelectionBeaconCommit,
    CommitteeSelectionBeaconPhase,
    CommitteeSelectionBeaconReveal,
    ExecuteTransactionError,
    ExecuteTransactionOptions,
    ExecuteTransactionRequest,
    ExecuteTransactionRetry,
    ExecuteTransactionWait,
    ExecutionError,
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
    signer: SignerSubmitTxSocket,
    app_query: c!(C::ApplicationInterface::SyncExecutor),
    node_index: NodeIndex,
}

impl<C: NodeComponents> CommitteeBeaconListener<C> {
    pub async fn new(
        db: RocksCommitteeBeaconDatabase,
        notifier: C::NotifierInterface,
        signer: SignerSubmitTxSocket,
        app_query: c!(C::ApplicationInterface::SyncExecutor),
        node_index: NodeIndex,
    ) -> Result<Self> {
        Ok(Self {
            db,
            notifier,
            signer,
            app_query,
            node_index,
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
            let Some(notification) = notification else {
                tracing::debug!("notifier is not running, shutting down");
                break;
            };
            let response = notification.response;

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

        // Get the current block number.
        let current_block = response.block_number + 1;

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
                    .handle_commit_phase(start_block, end_block, current_block)
                    .await;

                // Handling executed block responses is best-effort, and so if the commit phase
                // handler fails for this block notification, we should log an error and continue,
                // rather than returning an error and stopping the component.
                if let Err(e) = result {
                    tracing::error!("failed to handle commit phase: {:?}", e);
                }
            },
            Some(Value::CommitteeSelectionBeaconPhase(CommitteeSelectionBeaconPhase::Reveal(
                (start_block, end_block),
            ))) => {
                let result = self
                    .handle_reveal_phase(start_block, end_block, current_block)
                    .await;

                // Handling executed block responses is best-effort, and so if the reveal phase
                // handler fails for this block notification, we should log an error and continue,
                // rather than returning an error and stopping the component.
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
        current_block: BlockNumber,
    ) -> Result<(), CommitteeBeaconError> {
        tracing::debug!(
            "handling commit phase ({}, {}) at block {}",
            start_block,
            end_block,
            current_block
        );

        // If this is the first block outside of the commit phase range, execute a commit timeout
        // transaction and return.
        if current_block > end_block {
            tracing::debug!("submitting commit phase timeout transaction");
            let result = self
                .execute_transaction_with_no_retry(
                    UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout,
                )
                .await;

            if let Err(e) = result {
                // We don't need to return an error here because we expect this transaction to be
                // reverted for most nodes since we only need the first node to trigger the timeout.
                // It's also best-effort in that we'll just try again on the next block.
                tracing::debug!("failed to submit commit phase timeout transaction: {:?}", e);
            }

            return Ok(());
        }

        // If we are not in the commit phase block range, do nothing and return.
        if current_block < start_block || current_block > end_block {
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
        let reveal = self.generate_random_reveal();
        let epoch = self.app_query.get_current_epoch();
        let round = self.app_query.get_committee_selection_beacon_round();
        let Some(round) = round else {
            tracing::error!("no committee selection beacon round found for commit");
            return Ok(());
        };
        let commit = CommitteeSelectionBeaconCommit::build(epoch, round, reveal);

        // Save random beacon data to local database.
        self.db.set_beacon(commit, reveal);

        // Build commit and submit it.
        tracing::info!("submitting commit transaction: {:?}", commit);
        self.execute_transaction_with_retry_on_invalid_nonce(
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
        current_block: BlockNumber,
    ) -> Result<(), CommitteeBeaconError> {
        tracing::debug!(
            "handling reveal phase ({}, {}) at block {}",
            start_block,
            end_block,
            current_block
        );

        // If this is the first block outside of the reveal phase range, execute a reveal timeout
        // transaction and return. This will trigger an update in the application executor that will
        // restart a new round and commit phase.
        if current_block > end_block {
            tracing::debug!("submitting reveal phase timeout transaction");
            let result = self
                .execute_transaction_with_no_retry(
                    UpdateMethod::CommitteeSelectionBeaconRevealPhaseTimeout,
                )
                .await;

            if let Err(e) = result {
                // We don't need to return an error here because we expect this transaction to be
                // reverted for most nodes since we only need the first node to trigger the timeout.
                // It's also best-effort in that we'll just try again on the next block.
                tracing::debug!("failed to submit reveal phase timeout transaction: {:?}", e);
            }

            return Ok(());
        }

        // If we are not in the reveal phase block range, do nothing and return.
        if current_block < start_block || current_block > end_block {
            return Ok(());
        }

        // If we have not committed, there's nothing to reveal.
        let node_beacon = self
            .app_query
            .get_committee_selection_beacon(&self.node_index);
        let Some(node_beacon) = node_beacon else {
            tracing::debug!("node {} has not committed in reveal phase", self.node_index);
            return Ok(());
        };

        // If we have already revealed, there's nothing to do.
        let (commit, reveal) = node_beacon;
        if reveal.is_some() {
            tracing::debug!("node {} already revealed in reveal phase", self.node_index);
            return Ok(());
        }

        // Retrieve our beacon random data from local database.
        let Some(reveal) = self.db.query().get_beacon(commit) else {
            tracing::warn!("no beacon found for reveal in local database");
            return Ok(());
        };

        // Otherwise, send our reveal transaction.
        tracing::info!("submitting reveal transaction: {:?}", reveal);
        self.execute_transaction_with_retry_on_invalid_nonce(
            UpdateMethod::CommitteeSelectionBeaconReveal { reveal },
        )
        .await?;

        Ok(())
    }

    /// Execute transaction via the signer component.
    async fn execute_transaction(
        &self,
        method: UpdateMethod,
        options: ExecuteTransactionOptions,
    ) -> Result<(), ExecuteTransactionError> {
        self.signer
            .run(ExecuteTransactionRequest {
                method,
                options: Some(options),
            })
            .await??;
        Ok(())
    }

    async fn execute_transaction_with_retry_on_invalid_nonce(
        &self,
        method: UpdateMethod,
    ) -> Result<(), ExecuteTransactionError> {
        self.execute_transaction(
            method,
            ExecuteTransactionOptions {
                wait: ExecuteTransactionWait::Receipt,
                retry: ExecuteTransactionRetry::OnlyWith((
                    None,
                    Some(vec![ExecutionError::InvalidNonce]),
                    true,
                )),
                ..Default::default()
            },
        )
        .await
    }

    async fn execute_transaction_with_no_retry(
        &self,
        method: UpdateMethod,
    ) -> Result<(), ExecuteTransactionError> {
        self.execute_transaction(
            method,
            ExecuteTransactionOptions {
                wait: ExecuteTransactionWait::Receipt,
                retry: ExecuteTransactionRetry::Never,
                ..Default::default()
            },
        )
        .await
    }

    /// Generate random reveal value.
    fn generate_random_reveal(&self) -> CommitteeSelectionBeaconReveal {
        let mut rng = rand::thread_rng();
        let mut reveal = [0u8; 32];
        rng.fill(&mut reveal);
        reveal
    }
}
