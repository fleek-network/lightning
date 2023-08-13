use std::time::Duration;

use async_trait::async_trait;
use infusion::infu;
use tokio::sync::mpsc;

use crate::{application::SyncQueryRunnerInterface, infu_collection::Collection};

#[derive(Debug)]
pub enum Notification {
    NewEpoch,
    BeforeEpochChange,
}

#[async_trait]
pub trait NotifierInterface {
    infu!(NotifierInterface @ Input);

    // -- DYNAMIC TYPES
    type SyncQuery: SyncQueryRunnerInterface;

    // -- BOUNDED TYPES
    // empty

    fn init(query_runner: Self::SyncQuery) -> Self;

    fn notify_on_new_epoch(&self, tx: mpsc::Sender<Notification>);

    fn notify_before_epoch_change(&self, duration: Duration, tx: mpsc::Sender<Notification>);
}
