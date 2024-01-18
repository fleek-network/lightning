use std::sync::Arc;
use std::time::{Duration, SystemTime};

use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::notifier::{EpochNotifierEmitter, Notification, NotifierInterface};
use lightning_interfaces::ApplicationInterface;
use lightning_utils::application::QueryRunnerExt;
use tokio::sync::{mpsc, Notify};
use tokio::time::sleep;

pub struct Notifier<C: Collection> {
    query_runner: c![C::ApplicationInterface::SyncExecutor],
    epoch_notify: EpochChangeNotificationsEmitter,
}

impl<C: Collection> Clone for Notifier<C> {
    fn clone(&self) -> Self {
        Self {
            query_runner: self.query_runner.clone(),
            epoch_notify: self.epoch_notify.clone(),
        }
    }
}

impl<C: Collection> Notifier<C> {
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

impl<C: Collection> NotifierInterface<C> for Notifier<C> {
    type EpochEmitter = EpochChangeNotificationsEmitter;

    fn epoch_emitter(&self) -> Self::EpochEmitter {
        self.epoch_notify.clone()
    }

    fn init(app: &c![C::ApplicationInterface]) -> Self {
        Self {
            query_runner: app.sync_query(),
            epoch_notify: Default::default(),
        }
    }

    fn notify_on_new_epoch(&self, tx: mpsc::Sender<Notification>) {
        let notify = self.epoch_notify.clone();

        tokio::spawn(async move {
            loop {
                notify.emitter.notified().await;

                if tx.send(Notification::NewEpoch).await.is_err() {
                    // There is no receiver anymore.
                    return;
                }
            }
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

#[derive(Default, Clone)]
pub struct EpochChangeNotificationsEmitter {
    emitter: Arc<Notify>,
}

impl EpochNotifierEmitter for EpochChangeNotificationsEmitter {
    fn epoch_changed(&self) {
        self.emitter.notify_waiters()
    }
}

#[cfg(test)]
mod tests {
    use lightning_application::app::Application;
    use lightning_application::config::{Config, Mode, StorageConfig};
    use lightning_application::genesis::Genesis;
    use lightning_interfaces::application::{ApplicationInterface, ExecutionEngineSocket};
    use lightning_interfaces::infu_collection::Collection;
    use lightning_interfaces::partial;

    use super::*;

    partial!(TestBinding {
        ApplicationInterface = Application<Self>;
        NotifierInterface = Notifier<Self>;
    });

    const EPSILON: f64 = 0.1;

    fn init_app(epoch_time: u64) -> (ExecutionEngineSocket, Application<TestBinding>) {
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
            testnet: false,
            storage: StorageConfig::InMemory,
            db_path: None,
            db_options: None,
        };

        let app = Application::<TestBinding>::init(config, Default::default()).unwrap();

        (app.transaction_executor(), app)
    }

    // This take currently is broken since the application doesn't automatically move the epoch
    // forward. It needs a transaction to do it.
    //
    // #[tokio::test]
    // async fn test_on_new_epoch() {
    //     let (_, app) = init_app(2000);

    //     let notifier = Notifier::<TestBinding>::init(&app);

    //     // Request to be notified when the epoch ends.
    //     let (tx, mut rx) = mpsc::channel(2048);
    //     let now = SystemTime::now();
    //     notifier.notify_on_new_epoch(tx);

    //     // The epoch time is 2 secs, the notification will be send when the epoch ends,
    //     // hence, the notification should arrive approx. 2 secs after the request was made.
    //     if let Notification::NewEpoch = rx.recv().await.unwrap() {
    //         let elapsed_time = now.elapsed().unwrap();
    //         assert!((elapsed_time.as_secs_f64() - 2.0).abs() < EPSILON);
    //     }
    // }

    #[tokio::test]
    async fn test_before_epoch_change() {
        let (_, app) = init_app(3000);

        let notifier = Notifier::<TestBinding>::init(&app);

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
