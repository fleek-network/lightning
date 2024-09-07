use std::time::Duration;

use fdi::BuildGraph;
use lightning_types::{Block, BlockExecutionResponse};
use merklize::StateRootHash;

use crate::collection::Collection;

#[derive(Clone, Debug)]
pub struct BlockExecutedNotification {
    pub block: Block,
    pub response: BlockExecutionResponse,
}

#[derive(Clone, Debug)]
pub struct EpochChangedNotification {
    pub current_epoch: u64,
    pub last_epoch_hash: [u8; 32],
    pub previous_state_root: StateRootHash,
    pub new_state_root: StateRootHash,
}

/// # Notifier
#[interfaces_proc::blank]
pub trait NotifierInterface<C: Collection>: BuildGraph + Sync + Send + Clone {
    type Emitter: Emitter;

    /// Returns a reference to the emitter end of this notifier. Should only be used if we are
    /// interested (and responsible) for triggering a notification around new epoch.
    #[blank = Default::default()]
    fn get_emitter(&self) -> Self::Emitter;

    #[blank = crate::_hacks::Blanket]
    fn subscribe_block_executed(&self) -> impl Subscriber<BlockExecutedNotification>;

    #[blank = crate::_hacks::Blanket]
    fn subscribe_epoch_changed(&self) -> impl Subscriber<EpochChangedNotification>;

    #[blank = crate::_hacks::Blanket]
    fn subscribe_before_epoch_change(&self, duration: Duration) -> impl Subscriber<()>;
}

#[interfaces_proc::blank]
pub trait Emitter: Clone + Send + Sync + 'static {
    /// Notify the waiters about epoch change.
    fn epoch_changed(
        &self,
        epoch: u64,
        hash: [u8; 32],
        previous_state_root: StateRootHash,
        new_state_root: StateRootHash,
    );

    /// Notify the waiters about new block.
    fn new_block(&self, block: Block, response: BlockExecutionResponse);
}

#[interfaces_proc::blank]
pub trait Subscriber<T>: Send + Sync + 'static {
    /// Receive the next notification from this subscriber or returns `None` if we are shutting
    /// down.
    #[pending]
    async fn recv(&mut self) -> Option<T>;

    /// Jump to the last availabe notification skipping over any pending notifications that are
    /// ready.
    ///
    /// Using this method is similar to a `watch` channel.
    #[pending]
    async fn last(&mut self) -> Option<T>;
}
