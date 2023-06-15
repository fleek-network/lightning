use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use draco_application::query_runner::QueryRunner;
use draco_interfaces::{
    application::SyncQueryRunnerInterface,
    notifier::{Notification, NotifierInterface},
};
use tokio::{sync::mpsc, time::sleep};

pub struct Notifier {
    query_runner: <Notifier as NotifierInterface>::SyncQuery,
}

impl Notifier {
    fn get_until_epoch_end(&self) -> Duration {
        let epoch_info = self.query_runner.get_epoch_info();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let until_epoch_ends: u64 = (epoch_info.epoch_end as u128)
            .saturating_sub(now)
            .try_into()
            .unwrap();
        Duration::from_millis(until_epoch_ends)
    }
}

#[async_trait]
impl NotifierInterface for Notifier {
    type SyncQuery = QueryRunner;

    fn init(query_runner: Self::SyncQuery) -> Self {
        Self { query_runner }
    }

    fn notify_on_new_epoch(&self, tx: mpsc::Sender<Notification>) {
        let until_epoch_end = self.get_until_epoch_end();
        tokio::spawn(async move {
            sleep(until_epoch_end).await;
            tx.send(Notification::NewEpoch).await.unwrap();
        });
    }

    fn notify_before_epoch_change(&self, duration: Duration, tx: mpsc::Sender<Notification>) {
        let until_epoch_end = self.get_until_epoch_end();
        if until_epoch_end > duration {
            tokio::spawn(async move {
                sleep(until_epoch_end - duration).await;
                tx.send(Notification::BeforeEpochChange).await.unwrap();
            });
        }
    }
}
