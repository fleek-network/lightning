use std::ops::ControlFlow;
use std::sync::Mutex;

use futures::future::{select, Either};
use futures::Future;
use triomphe::Arc;

use crate::shared::{SharedState, NUM_SHARED_SHARDS};
use crate::wait_list::WaitList;
use crate::{OwnedShutdownSignal, ShutdownSignal};

/// The waiter end of a [ShutdownController] it can be cloned and passed around.
///
/// [ShutdownController]: crate::ShutdownController
pub struct ShutdownWaiter {
    inner: Arc<SharedState>,
    dedicated: Option<Arc<Mutex<WaitList>>>,
}

impl Clone for ShutdownWaiter {
    fn clone(&self) -> Self {
        let state = self.inner.clone();
        let mut waiter = ShutdownWaiter::new(state);
        if self.dedicated.is_some() {
            waiter.mark_busy();
        }
        waiter
    }
}

impl ShutdownWaiter {
    #[inline(always)]
    pub(crate) fn new(state: Arc<SharedState>) -> Self {
        Self {
            inner: state,
            dedicated: None,
        }
    }

    /// Mark the current waiter as a busy waiter. This will give the current waiter a dedicated
    /// space for the futures it needs which keeps the underlying mutex access localized only
    /// to this waiter.
    ///
    /// It is not recommended to call this function on a lot of waiters and is only recommended
    /// for when there are *many* calls to `wait_for_shutdown`.
    ///
    /// Also this function itself should be considered slow and should only be used at a setup
    /// stage.
    pub fn mark_busy(&mut self) {
        if self.dedicated.is_some() {
            return;
        }

        self.dedicated = self.inner.new_dedicated_wait_list();
    }

    /// Returns true if we have already shutdown.
    pub fn is_shutdown(&self) -> bool {
        let ptr = self as *const _ as *const () as usize as u64;
        let shard = self.dedicated.as_deref().unwrap_or_else(|| {
            let out = fxhash::hash32(&ptr) % (NUM_SHARED_SHARDS as u32);
            &self.inner.wait_list_shards[out as usize]
        });

        let wait_list = shard.lock().unwrap();
        wait_list.is_closed()
    }

    /// Standalone function to wait until the shutdown signal is received.
    /// This function is 100% cancel safe and will always return immediately
    /// if shutdown has already happened.
    #[inline(always)]
    pub fn wait_for_shutdown(&self) -> ShutdownSignal {
        ShutdownSignal::new(&self.inner, self.dedicated.as_deref())
    }

    /// Returns an owned future that will be resolved once shutdown is called.
    #[inline(always)]
    pub fn wait_for_shutdown_owned(&self) -> OwnedShutdownSignal {
        OwnedShutdownSignal::new(self.inner.clone(), self.dedicated.clone())
    }

    /// Create a new [OwnedShutdownSignal] from this waiter.
    #[inline(always)]
    pub fn into_future(self) -> OwnedShutdownSignal {
        Self::into(self)
    }

    /// Run a function until a shutdown signal is received.
    ///
    /// This method is recommended to use for run loops that are spawned, since the notify permit
    /// will persist then entire time until the run loop future resolves, and will be polled any
    /// time the run loop yields back to the async executor allowing very fast immediate exits.
    ///
    /// This should be considered on a case by case basis, as sometimes it's desirable to fully
    /// handle a branch before checking and exiting on shutdown. For example, maybe a piece of
    /// code than handles a write ahead log on disk might need to be ensured to always complete
    /// if it's doing work, so that no items are lost during a shutdown.
    pub async fn run_until_shutdown<T>(&self, fut: impl Future<Output = T>) -> Option<T> {
        let future1 = self.wait_for_shutdown();
        let future2 = std::pin::pin!(fut);
        match select(future1, future2).await {
            Either::Left(_) => None,
            Either::Right((r, _)) => Some(r),
        }
    }

    #[inline]
    pub async fn fold_until_shutdown<F, O, T, R>(&self, init: T, mut task: F) -> Option<R>
    where
        F: FnMut(T) -> O,
        O: Future<Output = ControlFlow<R, T>>,
    {
        let mut notified = self.wait_for_shutdown();
        let mut current = init;

        loop {
            let output = std::pin::pin!(task(current));

            match select(notified, output).await {
                Either::Left(_) => return None,
                Either::Right((ControlFlow::Break(ret), _)) => {
                    return Some(ret);
                },
                Either::Right((ControlFlow::Continue(new), tmp)) => {
                    current = new;
                    notified = tmp;
                },
            }
        }
    }

    #[inline]
    pub async fn run_task_until_shutdown<F, O, R>(&self, mut task: F) -> Option<R>
    where
        F: FnMut() -> O,
        O: Future<Output = ControlFlow<R>>,
    {
        self.fold_until_shutdown((), |_| task()).await
    }
}

impl From<ShutdownWaiter> for OwnedShutdownSignal {
    fn from(waiter: ShutdownWaiter) -> Self {
        OwnedShutdownSignal::new(waiter.inner, waiter.dedicated)
    }
}
