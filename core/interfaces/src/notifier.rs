use std::time::Duration;

use infusion::{c, ok};
use tokio::sync::mpsc;

use crate::infu_collection::Collection;
use crate::ApplicationInterface;

#[derive(Debug)]
pub enum Notification {
    NewEpoch,
    BeforeEpochChange,
}

#[infusion::service]
pub trait NotifierInterface<C: Collection>: Sync + Send + Clone {
    type EpochEmitter: EpochNotifierEmitter + Send + Sync + Clone;
    // type BlockEmitter: BlockNotifierEmitter;

    fn _init(app: ::ApplicationInterface) {
        ok!(Self::init(app))
    }

    fn init(app: &c!(C::ApplicationInterface)) -> Self;

    /// Returns a reference to the emitter end of this notifier. Should only be used if we are
    /// interested (and responsible) for triggering a notification around new epoch.
    fn epoch_emitter(&self) -> Self::EpochEmitter;

    // fn notify_on_new_block(&self, tx: Sender<Notification>);

    fn notify_on_new_epoch(&self, tx: mpsc::Sender<Notification>);

    fn notify_before_epoch_change(&self, duration: Duration, tx: mpsc::Sender<Notification>);
}

#[infusion::blank]
pub trait EpochNotifierEmitter {
    /// Notify the waiters about epoch change.
    fn epoch_changed(&self);
}
