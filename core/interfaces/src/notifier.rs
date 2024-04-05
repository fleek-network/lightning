use std::time::Duration;

use fdi::BuildGraph;
use tokio::sync::mpsc;

use crate::infu_collection::Collection;

#[derive(Debug, PartialEq)]
pub enum Notification {
    NewBlock,
    NewEpoch,
    BeforeEpochChange,
}

#[infusion::service]
pub trait NotifierInterface<C: Collection>: BuildGraph + Sync + Send + Clone {
    type Emitter: Emitter;

    /// Returns a reference to the emitter end of this notifier. Should only be used if we are
    /// interested (and responsible) for triggering a notification around new epoch.
    fn get_emitter(&self) -> Self::Emitter;

    fn notify_on_new_block(&self, tx: mpsc::Sender<Notification>);

    fn notify_on_new_epoch(&self, tx: mpsc::Sender<Notification>);

    fn notify_before_epoch_change(&self, duration: Duration, tx: mpsc::Sender<Notification>);
}

#[infusion::blank]
pub trait Emitter: Clone + Send + Sync + 'static {
    /// Notify the waiters about epoch change.
    fn epoch_changed(&self);

    /// Notify the waiters about new block.
    fn new_block(&self);
}
