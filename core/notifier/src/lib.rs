use std::sync::Arc;
use std::time::{Duration, SystemTime};

use lightning_interfaces::fdi::{BuildGraph, DependencyGraph};
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::notifier::{Notification, NotifierInterface};
use lightning_interfaces::types::{Block, BlockExecutionResponse};
use lightning_interfaces::{
    ApplicationInterface,
    BlockExecutedNotification,
    Cloned,
    Emitter,
    ShutdownWaiter,
    Subscriber,
};
use lightning_utils::application::QueryRunnerExt;
use tokio::pin;
use tokio::sync::{broadcast, mpsc, Notify};
use tokio::time::sleep;

pub struct Notifier<C: Collection> {
    query_runner: c![C::ApplicationInterface::SyncExecutor],
    notify: NotificationsEmitter,
    waiter: ShutdownWaiter,
}

impl<C: Collection> Clone for Notifier<C> {
    fn clone(&self) -> Self {
        Self {
            query_runner: self.query_runner.clone(),
            notify: self.notify.clone(),
            waiter: self.waiter.clone(),
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

impl<C: Collection> Notifier<C> {
    fn new(app: &c![C::ApplicationInterface], Cloned(waiter): Cloned<ShutdownWaiter>) -> Self {
        Self {
            query_runner: app.sync_query(),
            notify: NotificationsEmitter::default(),
            waiter,
        }
    }
}

impl<C: Collection> BuildGraph for Notifier<C> {
    fn build_graph() -> DependencyGraph {
        DependencyGraph::new().with_infallible(Self::new)
    }
}

impl<C: Collection> NotifierInterface<C> for Notifier<C> {
    type Emitter = NotificationsEmitter;

    fn get_emitter(&self) -> Self::Emitter {
        self.notify.clone()
    }

    fn subscribe_block_executed(&self) -> impl Subscriber<BlockExecutedNotification> {
        BroadcastSub(self.notify.block_executed.subscribe())
    }

    fn notify_on_new_block(&self, tx: mpsc::Sender<Notification>) {
        let notify = self.notify.clone();
        let waiter = self.waiter.clone();

        tokio::spawn(async move {
            let shutdown_future = waiter.wait_for_shutdown();
            pin!(shutdown_future);

            loop {
                let notify_future = notify.new_block_notify.notified();
                pin!(notify_future);

                tokio::select! {
                    _ = &mut shutdown_future => {
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
        let waiter = self.waiter.clone();

        tokio::spawn(async move {
            let shutdown_future = waiter.wait_for_shutdown();
            pin!(shutdown_future);

            loop {
                let notify_future = notify.new_epoch_notify.notified();
                pin!(notify_future);

                tokio::select! {
                    _ = &mut shutdown_future => {
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
        // TODO(qti3e): The name of this method is misleading, the other methods subscribe
        // to an event but this method only emits one thing and returns and yet it take an
        // mpsc and not a oneshot.
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

#[derive(Clone)]
pub struct NotificationsEmitter {
    block_executed: broadcast::Sender<BlockExecutedNotification>,
    new_block_notify: Arc<Notify>,
    new_epoch_notify: Arc<Notify>,
}

impl Default for NotificationsEmitter {
    fn default() -> Self {
        Self {
            block_executed: broadcast::channel(32).0,
            new_block_notify: Default::default(),
            new_epoch_notify: Default::default(),
        }
    }
}

impl Emitter for NotificationsEmitter {
    fn new_block(&self, block: Block, response: BlockExecutionResponse) {
        self.new_block_notify.notify_waiters();

        // The send could only fail if there are no active listeners at the moment
        // which is something we don't really care about and is expected by us.
        let _ = self
            .block_executed
            .send(BlockExecutedNotification { block, response });
    }

    fn epoch_changed(&self) {
        self.new_epoch_notify.notify_waiters()
    }
}

/// Provides an implementation for [`Subscriber`] backed by a tokio broadcast.
struct BroadcastSub<T>(broadcast::Receiver<T>);

impl<T> Subscriber<T> for BroadcastSub<T>
where
    T: Clone + Send + Sync + 'static,
{
    async fn recv(&mut self) -> Option<T> {
        loop {
            match self.0.recv().await {
                Ok(item) => break Some(item),
                Err(broadcast::error::RecvError::Closed) => break None,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    }

    async fn last(&mut self) -> Option<T> {
        let mut maybe_last = None;

        loop {
            match self.0.try_recv() {
                Ok(item) => {
                    maybe_last = Some(item);
                    break;
                },
                // We missed a few message because of the channel capacity, trying to recv again
                // will return an item.
                Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
                // The channel is empty at this point so there is no point in try_recv any longer.
                Err(broadcast::error::TryRecvError::Empty) => break,
                // The sender is dropped what we may have in `maybe_last` is what there is so we
                // just return it.
                Err(broadcast::error::TryRecvError::Closed) => return maybe_last,
            }
        }

        if let Some(v) = maybe_last {
            // Means we exited the prev loop because of Error::Empty.
            return Some(v);
        }

        // Await for the next message. Unless we are lagged behind due to channel capacity this
        // should only loop once.
        loop {
            match self.0.recv().await {
                Ok(item) => break Some(item),
                Err(broadcast::error::RecvError::Closed) => break None,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use lightning_application::app::Application;
//     use lightning_application::config::{Config, Mode, StorageConfig};
//     use lightning_application::genesis::Genesis;
//     use lightning_interfaces::application::{ApplicationInterface, ExecutionEngineSocket};
//     use lightning_interfaces::infu_collection::Collection;
//     use lightning_interfaces::partial;

//     use super::*;

//     partial!(TestBinding {
//         ApplicationInterface = Application<Self>;
//         NotifierInterface = Notifier<Self>;
//     });

//     const EPSILON: f64 = 0.1;

//     fn init_app(epoch_time: u64) -> (ExecutionEngineSocket, Application<TestBinding>) {
//         let mut genesis = Genesis::load().expect("Failed to load genesis from file.");
//         let epoch_start = SystemTime::now()
//             .duration_since(SystemTime::UNIX_EPOCH)
//             .unwrap()
//             .as_millis() as u64;
//         genesis.epoch_start = epoch_start;
//         genesis.epoch_time = epoch_time;
//         let config = Config {
//             genesis: Some(genesis),
//             mode: Mode::Test,
//             testnet: false,
//             storage: StorageConfig::InMemory,
//             db_path: None,
//             db_options: None,
//         };

//         let app = Application::<TestBinding>::init(config, Default::default()).unwrap();

//         (app.transaction_executor(), app)
//     }

//     #[tokio::test]
//     async fn test_before_epoch_change() {
//         let (_, app) = init_app(3000);

//         let notifier = Notifier::<TestBinding>::new(&app);

//         // Request to be notified 1 sec before the epoch ends.
//         let (tx, mut rx) = mpsc::channel(2048);
//         let now = SystemTime::now();
//         notifier.notify_before_epoch_change(Duration::from_secs(1), tx);

//         // The epoch time is 3 secs, the notification will be send 1 sec before the epoch ends,
//         // hence, the notification should arrive approx. 2 secs after the request was made.
//         if let Notification::BeforeEpochChange = rx.recv().await.unwrap() {
//             let elapsed_time = now.elapsed().unwrap();
//             assert!((elapsed_time.as_secs_f64() - 2.0).abs() < EPSILON);
//         }
//     }

//     #[tokio::test]
//     async fn test_notify_on_epoch_change() {
//         let (_, app) = init_app(3000);

//         let notifier = Notifier::<TestBinding>::new(&app);

//         // Request to be notified about new epoch.
//         let (tx, mut rx) = mpsc::channel(10);
//         notifier.notify_on_new_epoch(tx);

//         // Trigger new epoch Notification
//         tokio::spawn(async move {
//             sleep(Duration::from_secs(1)).await;
//             notifier.get_emitter().epoch_changed();
//         });

//         assert_eq!(Notification::NewEpoch, rx.recv().await.unwrap());
//     }

//     #[tokio::test]
//     async fn test_notify_on_new_block() {
//         let (_, app) = init_app(3000);

//         let notifier = Notifier::<TestBinding>::new(&app);

//         // Request to be notified about new block.
//         let (tx, mut rx) = mpsc::channel(10);
//         notifier.notify_on_new_block(tx);

//         // Trigger new block Notification
//         tokio::spawn(async move {
//             sleep(Duration::from_secs(1)).await;
//             notifier.get_emitter().new_block();
//         });

//         assert_eq!(Notification::NewBlock, rx.recv().await.unwrap());
//     }
// }

#[tokio::test]
async fn demo() {
    let (tx, mut rx) = broadcast::channel(3);
    tx.send(0).unwrap();
    tx.send(1).unwrap();
    tx.send(2).unwrap();
    tx.send(3).unwrap();
    tx.send(4).unwrap();
    dbg!(rx.recv().await);
    dbg!(rx.recv().await);
}
