use std::time::Duration;

use fdi::BuildGraph;
use infusion::Blank;
use lightning_types::{Block, BlockExecutionResponse};

use crate::infu_collection::Collection;

#[derive(Clone, Debug)]
pub struct BlockExecutedNotification {
    pub block: Block,
    pub response: BlockExecutionResponse,
}

#[derive(Clone, Debug)]
pub struct EpochChangedNotification {
    pub current_epoch: u64,
    pub last_epoch_hash: [u8; 32],
}

/// # Notifier
#[infusion::service]
pub trait NotifierInterface<C: Collection>: BuildGraph + Sync + Send + Clone {
    type Emitter: Emitter;

    /// Returns a reference to the emitter end of this notifier. Should only be used if we are
    /// interested (and responsible) for triggering a notification around new epoch.
    #[blank = Default::default()]
    fn get_emitter(&self) -> Self::Emitter;

    #[blank = Blank::default()]
    fn subscribe_block_executed(&self) -> impl Subscriber<BlockExecutedNotification>;

    #[blank = Blank::default()]
    fn subscribe_epoch_changed(&self) -> impl Subscriber<EpochChangedNotification>;

    #[blank = Blank::default()]
    fn subscribe_before_epoch_change(&self, duration: Duration) -> impl Subscriber<()>;
}

#[infusion::blank]
pub trait Emitter: Clone + Send + Sync + 'static {
    /// Notify the waiters about epoch change.
    fn epoch_changed(&self, epoch: u64, hash: [u8; 32]);

    /// Notify the waiters about new block.
    fn new_block(&self, block: Block, response: BlockExecutionResponse);
}

#[infusion::blank]
pub trait Subscriber<T>: Send + Sync + 'static {
    /// Receive the next notification from this subscriber or returns `None` if we are shutting
    /// down.
    async fn recv(&mut self) -> Option<T>;

    /// Jump to the last availabe notification skipping over any pending notifications that are
    /// ready.
    ///
    /// Using this method is similar to a `watch` channel.
    async fn last(&mut self) -> Option<T>;
}
