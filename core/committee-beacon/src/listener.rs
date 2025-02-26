use anyhow::Result;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::CommitteeSelectionBeaconRound;
use lightning_utils::application::QueryRunnerExt;
use rand::Rng;
use types::{
    BlockExecutionResponse,
    CommitteeSelectionBeaconCommit,
    CommitteeSelectionBeaconPhase,
    CommitteeSelectionBeaconReveal,
    Epoch,
    ExecuteTransactionError,
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

        let epoch = self.app_query.get_current_epoch();

        // Clear the local database after epoch change is executed.
        if response.change_epoch {
            // Clearing the beacons at epoch change is best-effort, since we can't guarantee that
            // the notification will be received on time or at all, or that the listener will be
            // running. This is fine, since the beacons will be cleared on the next committee
            // selection phase anyway, and we don't rely on it for correctness. We also keep beacons
            // from the most recent epoch to avoid clearing beacons for the current epoch in case of
            // delayed notification.
            if epoch > 0 {
                tracing::debug!("clearing beacons at epoch change");
                self.db.clear_beacons_before_epoch(epoch);
            }
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
                (commit_phase_epoch, round),
            ))) => {
                let result = self
                    .handle_commit_phase(epoch, commit_phase_epoch, round)
                    .await;

                // Handling executed block responses is best-effort, and so if the commit phase
                // handler fails for this block notification, we should log an error and continue,
                // rather than returning an error and stopping the component.
                if let Err(e) = result {
                    tracing::error!("failed to handle commit phase: {:?}", e);
                }
            },
            Some(Value::CommitteeSelectionBeaconPhase(CommitteeSelectionBeaconPhase::Reveal(
                (reveal_phase_epoch, round),
            ))) => {
                let result = self
                    .handle_reveal_phase(epoch, reveal_phase_epoch, round)
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
        epoch: Epoch,
        commit_phase_epoch: Epoch,
        round: CommitteeSelectionBeaconRound,
    ) -> Result<(), CommitteeBeaconError> {
        tracing::debug!(
            "handling commit phase (epoch: {}, commit_phase_epoch: {})",
            epoch,
            commit_phase_epoch,
        );

        // If this node was a recent non-revealing node, skip the commit phase, since it will be
        // rejected/reverted anyway.
        let non_revealing_nodes = self
            .app_query
            .get_committee_selection_beacon_non_revealing_nodes();
        if non_revealing_nodes.contains(&self.node_index) {
            tracing::debug!(
                "node {} is non-revealing in previous epoch, skipping commit",
                self.node_index
            );
            return Ok(());
        }

        // If we are not in the correct commit phase, do nothing and return.
        if epoch != commit_phase_epoch {
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

        // Check if we already generated a commit for the given epoch and round.
        // This can happen because this method could be called before the commit was stored on the
        // state. In that case we would not return on the check above.
        let commit = if let Some(commit) = self.db.query().get_commit(epoch, round) {
            commit
        } else {
            // Generate a random value for our commit transaction, save to
            // local database, and submit it.
            let reveal = self.generate_random_reveal();

            let commit = CommitteeSelectionBeaconCommit::build(epoch, round, reveal);

            // Save random beacon data to local database.
            self.db.set_beacon(epoch, round, commit, reveal);
            commit
        };

        // Build commit and submit it.
        tracing::info!(
            "Submitting commit transaction (epoch: {epoch}, round: {round}): {:?}",
            commit
        );

        // TODO(matthias): check if transaction was ordered
        self.execute_transaction(UpdateMethod::CommitteeSelectionBeaconCommit { commit })
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
        epoch: Epoch,
        reveal_phase_epoch: Epoch,
        round: CommitteeSelectionBeaconRound,
    ) -> Result<(), CommitteeBeaconError> {
        tracing::debug!(
            "handling reveal phase (epoch: {}, reveal_phase_epoch: {})",
            epoch,
            reveal_phase_epoch,
        );

        // If we are not in the correct reveal phase, do nothing and return.
        if epoch != reveal_phase_epoch {
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
        let Some(reveal) = self.db.query().get_beacon(epoch, commit) else {
            tracing::warn!("no beacon found for reveal in local database");
            return Ok(());
        };

        // Otherwise, send our reveal transaction.
        tracing::info!(
            "Submitting reveal transaction (epoch: {epoch}, round: {round}): {:?}",
            reveal
        );

        // TODO(matthias): check if transaction was ordered
        self.execute_transaction(UpdateMethod::CommitteeSelectionBeaconReveal { reveal })
            .await?;

        Ok(())
    }

    /// Execute transaction via the signer component.
    async fn execute_transaction(
        &self,
        method: UpdateMethod,
    ) -> Result<(), ExecuteTransactionError> {
        self.signer
            .run(method)
            .await
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
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
