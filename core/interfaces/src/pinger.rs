use fdi::BuildGraph;
use ready::empty::EmptyReadyState;
use ready::ReadyWaiterState;

use crate::components::NodeComponents;

#[interfaces_proc::blank]
pub trait PingerInterface<C: NodeComponents>: BuildGraph + Sized + Send + Sync {
    #[blank(EmptyReadyState)]
    type ReadyState: ReadyWaiterState;

    /// Wait for pinger to be ready with the listen address.
    async fn wait_for_ready(&self) -> Self::ReadyState;
}
