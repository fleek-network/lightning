use std::sync::Arc;
use std::time::{Duration, SystemTime};

use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::notifier::{Notification, NotifierInterface};
use lightning_interfaces::{ApplicationInterface, Emitter};
use lightning_utils::application::QueryRunnerExt;
use tokio::pin;
use tokio::sync::{mpsc, Notify};
use tokio::time::sleep;

pub struct Notifier<C: Collection> {
    query_runner: c![C::ApplicationInterface::SyncExecutor],
    notify: NotificationsEmitter,
}

impl<C: Collection> Clone for Notifier<C> {
    fn clone(&self) -> Self {
        Self {
            query_runner: self.query_runner.clone(),
            notify: self.notify.clone(),
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
    type Emitter = NotificationsEmitter;

    fn get_emitter(&self) -> Self::Emitter {
        self.notify.clone()
    }

    fn init(app: &c![C::ApplicationInterface]) -> Self {
        Self {
            query_runner: app.sync_query(),
            notify: Default::default(),
        }
    }

    fn notify_on_new_block(&self, tx: mpsc::Sender<Notification>) {
        let notify = self.notify.clone();
        tokio::spawn(async move {
            loop {
                let notify_future = notify.new_block_notify.notified();
                let shutdown_future = notify.shutdown_notify.notified();
                pin!(shutdown_future);
                pin!(notify_future);

                tokio::select! {
                _ = shutdown_future => {
                    break;
                }
                _ = notify_future => {
                    if tx.send(Notification::NewBlock).await.is_err() {
                        // There is no receiver anymore.
                        return;
                    }
                }
                }
            }
        });
    }

    fn notify_on_new_epoch(&self, tx: mpsc::Sender<Notification>) {
        let notify = self.notify.clone();
        tokio::spawn(async move {
            loop {
                let notify_future = notify.new_epoch_notify.notified();
                let shutdown_future = notify.shutdown_notify.notified();
                pin!(shutdown_future);
                pin!(notify_future);

                tokio::select! {
                _ = shutdown_future => {
                    break;
                }
                _ = notify_future => {
                    if tx.send(Notification::NewEpoch).await.is_err() {
                        // There is no receiver anymore.
                        return;
                    }
                }
                }
            }
        });
    }

    fn notify_before_epoch_change(&self, duration: Duration, tx: mpsc::Sender<Notification>) {
        let until_epoch_end = self.get_until_epoch_end();
        if until_epoch_end > duration {
            tokio::spawn(async move {
                sleep(until_epoch_end - duration).await;
                tx.send(Notification::BeforeEpochChange)
                    .await
                    .expect("Failed to send notification before epoch change.")
            });
        }
    }
}

#[derive(Default, Clone)]
pub struct NotificationsEmitter {
    new_block_notify: Arc<Notify>,
    new_epoch_notify: Arc<Notify>,
    shutdown_notify: Arc<Notify>,
}

impl Emitter for NotificationsEmitter {
    fn new_block(&self) {
        self.new_block_notify.notify_waiters()
    }

    fn epoch_changed(&self) {
        self.new_epoch_notify.notify_waiters()
    }

    fn shutdown(&self) {
        self.shutdown_notify.notify_waiters()
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

    #[tokio::test]
    async fn test_notify_on_epoch_change() {
        let (_, app) = init_app(3000);

        let notifier = Notifier::<TestBinding>::init(&app);

        // Request to be notified about new epoch.
        let (tx, mut rx) = mpsc::channel(10);
        notifier.notify_on_new_epoch(tx);

        // Trigger new epoch Notification
        tokio::spawn(async move {
            sleep(Duration::from_secs(1)).await;
            notifier.get_emitter().epoch_changed();
        });

        assert_eq!(Notification::NewEpoch, rx.recv().await.unwrap());
    }

    #[tokio::test]
    async fn test_notify_on_new_block() {
        let (_, app) = init_app(3000);

        let notifier = Notifier::<TestBinding>::init(&app);

        // Request to be notified about new block.
        let (tx, mut rx) = mpsc::channel(10);
        notifier.notify_on_new_block(tx);

        // Trigger new block Notification
        tokio::spawn(async move {
            sleep(Duration::from_secs(1)).await;
            notifier.get_emitter().new_block();
        });

        assert_eq!(Notification::NewBlock, rx.recv().await.unwrap());
    }
}
