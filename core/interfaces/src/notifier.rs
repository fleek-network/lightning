use std::time::Duration;

use infusion::{infu, ok, p};
use tokio::sync::mpsc;

use crate::{infu_collection::Collection, ApplicationInterface};

#[derive(Debug)]
pub enum Notification {
    NewEpoch,
    BeforeEpochChange,
}

#[infusion::blank]
pub trait NotifierInterface: Clone {
    infu!(NotifierInterface, {
        fn init(app: ApplicationInterface) {
            ok!(Self::init(app.sync_query()))
        }
    });

    fn init(query_runner: p!(::ApplicationInterface::SyncExecutor)) -> Self;

    fn notify_on_new_epoch(&self, tx: mpsc::Sender<Notification>);

    fn notify_before_epoch_change(&self, duration: Duration, tx: mpsc::Sender<Notification>);
}
