use std::time::Duration;

use anyhow::{Context, Result};
use lightning_interfaces::prelude::*;
use lightning_utils::application::QueryRunnerExt;
use types::{
    CommitteeSelectionBeaconPhase,
    ExecuteTransactionError,
    ExecuteTransactionOptions,
    ExecuteTransactionRequest,
    ExecuteTransactionRetry,
    ExecuteTransactionWait,
    Metadata,
    TransactionResponse,
    UpdateMethod,
    Value,
};

use crate::{CommitteeBeaconError, CommitteeBeaconTimerConfig};

/// The committee beacon timer is responsible for ensuring that time (blocks) move forward during
/// commit and reveal phases.
///
/// This is done by watching the phase and block number metadata in the application state, and
/// submitting new benign transactions to the mempool to move the phase forward if necessary.
///
/// Most of the time in real world usage, transactions are being submitted by other actors,
/// but this is necessary to ensure that the phase advances if no transactions are submitted.
pub struct CommitteeBeaconTimer<C: NodeComponents> {
    config: CommitteeBeaconTimerConfig,
    app_query: c!(C::ApplicationInterface::SyncExecutor),
    signer: SignerSubmitTxSocket,
}

impl<C: NodeComponents> CommitteeBeaconTimer<C> {
    pub async fn new(
        config: CommitteeBeaconTimerConfig,
        signer: SignerSubmitTxSocket,
        app_query: c!(C::ApplicationInterface::SyncExecutor),
    ) -> Result<Self> {
        Ok(Self {
            config,
            app_query,
            signer,
        })
    }
    /// Start the committee beacon timer.
    ///
    /// Every tick of the timer, we check if the block number has advanced and that we are in the
    /// commit or reveal phase. If the block number has not advanced, we submit a benign
    /// transaction to move the phase forward.
    pub async fn start(&self) -> Result<(), CommitteeBeaconError> {
        let base_delay = self.config.tick_delay;
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
                        .execute_transaction_with_no_retry(UpdateMethod::IncrementNonce {})
                        .await;
                    match result {
                        Ok(_) => {
                            tracing::debug!(
                                "successfully executed tick transaction to advance phase (block: {})",
                                block_number
                            );
                        },
                        Err(ExecuteTransactionError::Reverted((tx, receipt, _))) => {
                            match receipt.response {
                                TransactionResponse::Success(_) => {
                                    // This should never be returned as an error, and is returned
                                    // with Ok(), so we can ignore it here.
                                },
                                TransactionResponse::Revert(err) => {
                                    tracing::debug!(
                                        "ignoring timer tick transaction revert (tx: {:?}): {:?}",
                                        tx.hash(),
                                        err
                                    );
                                },
                            }
                        },
                        Err(e) => {
                            tracing::error!("ignoring transaction client error: {:?}", e);
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

    /// Execute transaction via the signer component.
    async fn execute_transaction_with_no_retry(
        &self,
        method: UpdateMethod,
    ) -> Result<(), ExecuteTransactionError> {
        self.signer
            .run(ExecuteTransactionRequest {
                method,
                options: Some(ExecuteTransactionOptions {
                    // No need to retry the tick transaction, since it is best-effort, and all we
                    // need is any single node to do this to advance the phase.
                    retry: ExecuteTransactionRetry::Never,
                    wait: ExecuteTransactionWait::Receipt,
                    timeout: None,
                }),
            })
            .await??;
        Ok(())
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
