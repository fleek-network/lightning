use async_trait::async_trait;
use lightning_interfaces::SyncQueryRunnerInterface;
use lightning_utils::application::QueryRunnerExt;
use narwhal_executor::ExecutionState;
use narwhal_types::ConsensusOutput;
use tokio::sync::mpsc::Sender;

pub struct Execution<Q: SyncQueryRunnerInterface> {
    tx: Sender<ConsensusOutput>,
    query_runner: Q,
}

impl<Q: SyncQueryRunnerInterface> Execution<Q> {
    pub fn new(tx: Sender<ConsensusOutput>, query_runner: Q) -> Self {
        Self { tx, query_runner }
    }
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface> ExecutionState for Execution<Q> {
    async fn handle_consensus_output(&mut self, consensus_output: ConsensusOutput) {
        self.tx
            .send(consensus_output)
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
