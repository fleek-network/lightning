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
    fn _init(app: ::ApplicationInterface) {
        ok!(Self::init(app))
    }

    type Emitters: NotifierEmitters;

    fn init(app: &c!(C::ApplicationInterface)) -> Self;

    /// Returns a reference to the emitter end of this notifier. Should only be used if we are
    /// interested (and responsible) for triggering a notification.
    fn emitters(&self) -> &Self::Emitters;

    fn notify_on_new_epoch(&self, tx: mpsc::Sender<Notification>);

    fn notify_before_epoch_change(&self, duration: Duration, tx: mpsc::Sender<Notification>);
}


#[infusion::blank]
pub trait NotifierEmitters {
    /// Notify the waiters about epoch change.
    fn epoch_changed(&self);
}