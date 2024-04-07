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
    EpochChangedNotification,
    ShutdownWaiter,
    Subscriber,
};
use lightning_utils::application::QueryRunnerExt;
use tokio::sync::{broadcast, mpsc};
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

    fn subscribe_epoch_changed(&self) -> impl Subscriber<EpochChangedNotification> {
        BroadcastSub(self.notify.epoch_changed.subscribe())
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
    epoch_changed: broadcast::Sender<EpochChangedNotification>,
}

impl Default for NotificationsEmitter {
    fn default() -> Self {
        Self {
            block_executed: broadcast::channel(64).0,
            epoch_changed: broadcast::channel(16).0,
        }
    }
}

impl Emitter for NotificationsEmitter {
    fn new_block(&self, block: Block, response: BlockExecutionResponse) {
        // The send could only fail if there are no active listeners at the moment
        // which is something we don't really care about and is expected by us.
        let _ = self
            .block_executed
            .send(BlockExecutedNotification { block, response });
    }

    fn epoch_changed(&self, current_epoch: u64, last_epoch_hash: [u8; 32]) {
        let _ = self.epoch_changed.send(EpochChangedNotification {
            current_epoch,
            last_epoch_hash,
        });
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
