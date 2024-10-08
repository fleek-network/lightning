use std::collections::HashMap;

use fdi::BuildGraph;
use lightning_types::{AggregateCheckpoint, CheckpointAttestation, Epoch, NodeIndex};
use ready::empty::EmptyReadyState;
use ready::ReadyWaiterState;

use crate::components::NodeComponents;
use crate::ConfigConsumer;

/// The checkpointer is a component that produces checkpoint attestations and aggregates them when a
/// supermajority is reached.
///
/// It listens for epoch change notifications from the notifier and checkpoint attestation messages
/// from the broadcaster. It's responsible for creating checkpoint attestations and checking for a
/// supermajority of attestations for the epoch, aggregating the signatures if a supermajority is
/// reached, and saving the aggregate checkpoint to the database as the canonical checkpoint for the
/// epoch. The aggregate checkpoint contains a state root that can be used by clients to verify the
/// blockchain state using merkle proofs.
#[interfaces_proc::blank]
pub trait CheckpointerInterface<C: NodeComponents>:
    BuildGraph + ConfigConsumer + Send + Sync
{
    /// The ready state of the checkpointer.
    #[blank(EmptyReadyState)]
    type ReadyState: ReadyWaiterState;

    /// The query type for the checkpointer.
    type Query: CheckpointerQueryInterface;

    /// Get a query instance for the checkpointer.
    fn query(&self) -> Self::Query;

    /// Wait for the checkpointer to be ready after starting, that they are subscribed to the
    /// notifier and broadcaster.
    async fn wait_for_ready(&self) -> Self::ReadyState;
}

/// The query type for the checkpointer.
#[interfaces_proc::blank]
pub trait CheckpointerQueryInterface: Clone + Send + Sync + 'static {
    /// Get the set of checkpoint attestations for the given epoch.
    fn get_checkpoint_attestations(
        &self,
        epoch: Epoch,
    ) -> HashMap<NodeIndex, CheckpointAttestation>;

    /// Get the aggregate checkpoint for the given epoch.
    fn get_aggregate_checkpoint(&self, epoch: Epoch) -> Option<AggregateCheckpoint>;
}
