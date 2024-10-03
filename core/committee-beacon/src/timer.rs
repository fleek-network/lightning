use std::time::Duration;

use anyhow::{Context, Result};
use lightning_interfaces::prelude::*;
use lightning_utils::application::QueryRunnerExt;
use lightning_utils::transaction::{TransactionClient, TransactionClientError, TransactionSigner};
use types::{
    CommitteeSelectionBeaconPhase,
    ExecutionError,
    Metadata,
    TransactionReceipt,
    TransactionResponse,
    UpdateMethod,
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
    app_query: c!(C::ApplicationInterface::SyncExecutor),
    client: TransactionClient<C>,
}

impl<C: NodeComponents> CommitteeBeaconTimer<C> {
    pub async fn new(
        keystore: C::KeystoreInterface,
        notifier: C::NotifierInterface,
        app_query: c!(C::ApplicationInterface::SyncExecutor),
        mempool: MempoolSocket,
    ) -> Result<Self> {
        // Build a new transaction client for executing transactions signed by this node.
        let client = TransactionClient::new(
            app_query.clone(),
            notifier.clone(),
            mempool.clone(),
            TransactionSigner::NodeMain(keystore.get_ed25519_sk()),
        )
        .await;

        Ok(Self { app_query, client })
    }

    /// Start the committee beacon timer.
    ///
    /// Every tick of the timer, we check if the block number has advanced and that we are in the
    /// commit or reveal phase. If the block number has not advanced, we submit a benign
    /// transaction to move the phase forward.
    pub async fn start(&self) -> Result<(), CommitteeBeaconError> {
        let base_delay = Duration::from_millis(500);
        let mut current_delay = base_delay;
        let max_delay = Duration::from_secs(60);

        // The block number of the previous iteration of the loop.
        let mut prev_block_number = None;

        // The block number of the previous time we submitted a transaction to advance the phase.
        let mut prev_advance_block_number = None;

        loop {
            // Get latest block number in application state.
            let block_number = self
                .app_query
                .get_block_number()
                .context("failed to get block number")?;

            // Check if we need to advance the phase based on the block number and phase metadata.
            let mut is_advance_tick = false;
            if let Some(prev) = prev_block_number {
                // Check if the block number hasn't advanced and we're in the commit or reveal
                // phase.
                if block_number == prev && self.in_commit_or_reveal_phase() {
                    tracing::debug!(
                        "block number {} has not advanced since last tick, advancing phase",
                        block_number
                    );

                    // Run a benign transaction to advance the phase.
                    // Note that we do not retry or return error on invalid nonce revert here, since
                    // this is a best-effort mechanism to advance the phase. If it fails, the
                    // listener will just try again on the next tick, and likely means our
                    // goal of creating a block has already been achieved.
                    let result = self
                        .client
                        .execute_transaction(UpdateMethod::IncrementNonce {})
                        .await;
                    match result {
                        Ok(_) => {},
                        Err(TransactionClientError::Reverted((
                            _,
                            TransactionReceipt {
                                response: TransactionResponse::Revert(ExecutionError::InvalidNonce),
                                ..
                            },
                        ))) => {
                            tracing::debug!(
                                "ignoring timer tick transaction revert due to invalid nonce"
                            );
                        },
                        Err(e) => {
                            return Err(e.into());
                        },
                    }

                    // Indicate that this tick attempted to advance the phase.
                    is_advance_tick = true;
                }
            }

            // If the block number hasn't advanced since we last submitted a benign
            // transaction to get it to advance, something may be wrong. Apply exponential
            // back-off to the delay to avoid spamming the mempool.
            if let Some(prev_advance_block_number) = prev_advance_block_number {
                if block_number == prev_advance_block_number {
                    tracing::warn!(
                        "block number {} has not advanced since last advance tick, increasing delay",
                        block_number
                    );
                    current_delay = (current_delay * 2).min(max_delay);
                } else {
                    // Otherwise, reset the delay back to the base delay.
                    current_delay = base_delay;
                }
            }

            // Set the last advance block number if we attempted to advance the phase.
            if is_advance_tick {
                prev_advance_block_number = Some(block_number);
            }

            // Set our previous block number.
            prev_block_number = Some(block_number);

            // Sleep for the current tick delay.
            tokio::time::sleep(current_delay).await;
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
}
