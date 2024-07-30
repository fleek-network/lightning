use async_trait::async_trait;
use lightning_interfaces::SyncQueryRunnerInterface;
use lightning_utils::application::QueryRunnerExt;
use narwhal_executor::ExecutionState;
use narwhal_types::{Batch, ConsensusOutput, Transaction};
use tokio::sync::mpsc::Sender;
use tracing::error;

pub struct FilteredConsensusOutput {
    pub transactions: Vec<Transaction>,
    pub sub_dag_index: u64,
    pub sub_dag_round: u64,
}

pub struct Execution<Q: SyncQueryRunnerInterface> {
    tx: Sender<FilteredConsensusOutput>,
    query_runner: Q,
}

impl<Q: SyncQueryRunnerInterface> Execution<Q> {
    pub fn new(tx: Sender<FilteredConsensusOutput>, query_runner: Q) -> Self {
        Self { tx, query_runner }
    }
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface> ExecutionState for Execution<Q> {
    async fn handle_consensus_output(&mut self, consensus_output: ConsensusOutput) {
        let current_epoch = self.query_runner.get_current_epoch();
        let sub_dag_index = consensus_output.sub_dag.sub_dag_index;
        let sub_dag_round = consensus_output.sub_dag.leader.round();

        let batch_payload: Vec<Vec<u8>> = consensus_output
            .sub_dag
            .certificates
            .iter()
            .zip(consensus_output.batches)
            .filter_map(|(cert, batch)| {
                // Skip over the ones that have a different epoch. Shouldnt ever happen besides an
                // edge case towards the end of an epoch
                if cert.epoch() != current_epoch {
                    error!("we recieved a consensus cert from an epoch we are not on");
                    None
                } else {
                    // Map the batch to just the transactions
                    Some(
                        batch
                            .into_iter()
                            .flat_map(|batch| match batch {
                                // work around because batch.transactions() would require clone
                                Batch::V1(btch) => btch.transactions,
                                Batch::V2(btch) => btch.transactions,
                            })
                            .collect::<Vec<Vec<u8>>>(),
                    )
                }
            })
            .flatten()
            .collect();

        if batch_payload.is_empty() {
            return;
        }

        let filtered_output = FilteredConsensusOutput {
            transactions: batch_payload,
            sub_dag_index,
            sub_dag_round,
        };

        self.tx
            .send(filtered_output)
            .await
            .expect("Failed to send consensus output");
    }

    fn last_executed_sub_dag_round(&self) -> u64 {
        // The sub dag round is currently not used by Narwhal, so it's not clear if adding 1 is
        // necessary here as well.
        self.query_runner.get_sub_dag_round() + 1
    }

    fn last_executed_sub_dag_index(&self) -> u64 {
        // Note we add one here because of an off by 1 error in Narwhal codebase
        // if we actually return the last sub dag index that we exectuted during a restart that is
        // going to be the sub dag index they send us after a restart and we will re-execute it
        self.query_runner.get_sub_dag_index() + 1
    }
}
