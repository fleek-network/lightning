use fdi::BuildGraph;
use lightning_schema::LightningMessage;
use ready::empty::EmptyReadyState;
use ready::ReadyWaiterState;

use crate::components::NodeComponents;

#[interfaces_proc::blank]
pub trait ConsensusInterface<C: NodeComponents>: BuildGraph + Sized + Send + Sync {
    #[blank(())]
    type Certificate: LightningMessage + Clone;

    /// The ready state of the consensus component.
    #[blank(EmptyReadyState)]
    type ReadyState: ReadyWaiterState;

    /// Wait for the consensus component to be ready after starting.
    async fn wait_for_ready(&self) -> Self::ReadyState;
}
