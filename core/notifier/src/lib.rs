use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use freek_application::query_runner::QueryRunner;
use freek_interfaces::{
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

#[cfg(test)]
mod tests {
    use freek_application::{
        app::Application,
        config::{Config, Mode},
        genesis::Genesis,
        query_runner::QueryRunner,
    };
    use freek_interfaces::application::{ApplicationInterface, ExecutionEngineSocket};

    use super::*;

    const EPSILON: f64 = 0.1;

    async fn init_app(epoch_time: u64) -> (ExecutionEngineSocket, QueryRunner) {
        let mut genesis = Genesis::load().expect("Failed to load genesis from file.");
        let epoch_start = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        genesis.epoch_start = epoch_start;
        genesis.epoch_time = epoch_time;
        let config = Config {
            genesis: Some(genesis),
            mode: Mode::Test,
        };

        let app = Application::init(config).await.unwrap();

        (app.transaction_executor(), app.sync_query())
    }

    #[tokio::test]
    async fn test_on_new_epoch() {
        let (_, query_runner) = init_app(2000).await;

        let notifier = Notifier::init(query_runner);

        // Request to be notified when the epoch ends.
        let (tx, mut rx) = mpsc::channel(2048);
        let now = SystemTime::now();
        notifier.notify_on_new_epoch(tx);

        // The epoch time is 2 secs, the notification will be send when the epoch ends,
        // hence, the notification should arrive approx. 2 secs after the request was made.
        if let Notification::NewEpoch = rx.recv().await.unwrap() {
            let elapsed_time = now.elapsed().unwrap();
            assert!((elapsed_time.as_secs_f64() - 2.0).abs() < EPSILON);
        }
    }

    #[tokio::test]
    async fn test_before_epoch_change() {
        let (_, query_runner) = init_app(3000).await;

        let notifier = Notifier::init(query_runner);

        // Request to be notified 1 sec before the epoch ends.
        let (tx, mut rx) = mpsc::channel(2048);
        let now = SystemTime::now();
        notifier.notify_before_epoch_change(Duration::from_secs(1), tx);

        // The epoch time is 3 secs, the notification will be send 1 sec before the epoch ends,
        // hence, the notification should arrive approx. 2 secs after the request was made.
        if let Notification::BeforeEpochChange = rx.recv().await.unwrap() {
            let elapsed_time = now.elapsed().unwrap();
            assert!((elapsed_time.as_secs_f64() - 2.0).abs() < EPSILON);
        }
    }
}
