use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::future::{select, Either};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{Block, BlockExecutionResponse};
use lightning_interfaces::{
    BlockExecutedNotification,
    EpochChangedNotification,
    OwnedShutdownSignal,
};
use lightning_utils::application::QueryRunnerExt;
use merklize::StateRootHash;
use tokio::pin;
use tokio::sync::broadcast;
use tokio::time::sleep;

#[cfg(test)]
mod tests;

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
    fn new(
        app: &c![C::ApplicationInterface],
        fdi::Cloned(waiter): fdi::Cloned<ShutdownWaiter>,
    ) -> Self {
        Self {
            query_runner: app.sync_query(),
            notify: NotificationsEmitter::default(),
            waiter,
        }
    }
}

impl<C: Collection> BuildGraph for Notifier<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with_infallible(Self::new)
    }
}

impl<C: Collection> NotifierInterface<C> for Notifier<C> {
    type Emitter = NotificationsEmitter;

    fn get_emitter(&self) -> Self::Emitter {
        self.notify.clone()
    }

    fn subscribe_block_executed(&self) -> impl Subscriber<BlockExecutedNotification> {
        BroadcastSub(
            self.notify.block_executed.subscribe(),
            self.waiter.wait_for_shutdown_owned(),
        )
    }

    fn subscribe_epoch_changed(&self) -> impl Subscriber<EpochChangedNotification> {
        BroadcastSub(
            self.notify.epoch_changed.subscribe(),
            self.waiter.wait_for_shutdown_owned(),
        )
    }

    fn subscribe_before_epoch_change(&self, duration: Duration) -> impl Subscriber<()> {
        let (sender, rx) = broadcast::channel(8);
        let epoch_changed = BroadcastSub(
            self.notify.epoch_changed.subscribe(),
            self.waiter.wait_for_shutdown_owned(),
        );
        spawn!(
            before_epoch_change(sender, self.query_runner.clone(), duration, epoch_changed,),
            "NOTIFIER: subscribe before epoch change"
        );
        BroadcastSub(rx, self.waiter.wait_for_shutdown_owned())
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

    fn epoch_changed(
        &self,
        current_epoch: u64,
        last_epoch_hash: [u8; 32],
        previous_state_root: StateRootHash,
        new_state_root: StateRootHash,
    ) {
        let _ = self.epoch_changed.send(EpochChangedNotification {
            current_epoch,
            last_epoch_hash,
            previous_state_root,
            new_state_root,
        });
    }
}

/// Provides an implementation for [`Subscriber`] backed by a tokio broadcast.
pub(crate) struct BroadcastSub<T>(pub broadcast::Receiver<T>, pub OwnedShutdownSignal);

impl<T> Subscriber<T> for BroadcastSub<T>
where
    T: Clone + Send + Sync + 'static,
{
    async fn recv(&mut self) -> Option<T> {
        loop {
            let recv = self.0.recv();
            pin!(recv);

            match select(recv, &mut self.1).await {
                Either::Left((Ok(item), _)) => break Some(item),
                Either::Left((Err(broadcast::error::RecvError::Closed), _)) => {
                    break None;
                },
                Either::Left((Err(broadcast::error::RecvError::Lagged(_)), _)) => {
                    continue;
                },
                Either::Right(_) => return None,
            }
        }
    }

    async fn last(&mut self) -> Option<T> {
        let mut maybe_last = None;

        loop {
            match self.0.try_recv() {
                Ok(item) => {
                    maybe_last = Some(item);
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
        self.recv().await
    }
}

async fn before_epoch_change<Q>(
    sender: broadcast::Sender<()>,
    query_runner: Q,
    duration: Duration,
    mut epoch_changed: BroadcastSub<EpochChangedNotification>,
) where
    Q: SyncQueryRunnerInterface,
{
    loop {
        // We might start at the very end of an epoch or the sleep duration given to us might be
        // larger than the epoch duration (which should not happen), in these cases we want to skip
        // this epoch. This loop basically exits as soon as we have are at an epoch where we have
        // enough time to sleep.
        let sleep_fut = loop {
            if sender.receiver_count() == 0 {
                return;
            }

            if let Some(duration) = get_sleep_amount(duration, &query_runner) {
                break sleep(duration);
            }

            if epoch_changed.recv().await.is_none() {
                // pre-mature termination.
                return;
            }
        };

        pin!(sleep_fut);

        // Anticipate an unexpected epoch change anytime. In that case the current timer is void
        // and we start over again. We also exit on two conditions:
        // 1. If we fail to send a notification out.
        // 2. If the epoch change subscription returns none.
        tokio::select! {
            biased;
            _ = &mut sleep_fut => {
                if sender.send(()).is_err() {
                    return;
                }
            },
            changed = epoch_changed.recv() => {
                if changed.is_none() {
                    return;
                }

                continue;
            },
        }
    }
}

fn get_sleep_amount(
    duration: Duration,
    query_runner: &impl SyncQueryRunnerInterface,
) -> Option<Duration> {
    let now = now();
    let epoch_end = query_runner.get_epoch_info().epoch_end;

    let until_end = Duration::from_millis(epoch_end.saturating_sub(now));

    if duration < until_end {
        Some(until_end - duration)
    } else {
        None
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
