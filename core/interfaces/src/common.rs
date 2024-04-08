use std::future::Future;
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, Ordering};

pub use fdi::{Cloned, Consume, Ref, RefMut};
use futures::future::{select, Either};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::Notify;
use tracing::trace;
use triomphe::Arc;

struct ShutdownInner {
    notify: Notify,
    is_shutdown: AtomicBool,
    tx: UnboundedSender<()>,
}

/// Controller utility for shutdown
pub struct ShutdownController {
    inner: Arc<ShutdownInner>,
    rx: UnboundedReceiver<()>,
}

impl Default for ShutdownController {
    fn default() -> Self {
        let (tx, rx) = unbounded_channel();
        Self {
            inner: ShutdownInner {
                notify: Notify::default(),
                is_shutdown: false.into(),
                tx,
            }
            .into(),
            rx,
        }
    }
}

impl ShutdownController {
    /// Create a new shutdown inner.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a new waiter utility
    pub fn waiter(&self) -> ShutdownWaiter {
        ShutdownWaiter(self.inner.clone())
    }

    /// Trigger the shutdown signal and wait for all the child [`ShutdownWaiter`]'s are dropped.
    /// This function WILL hang if any waiters are not correctly dropped on shutdown.
    pub async fn shutdown(&mut self) {
        // Set the shutdown boolean to true, preventing any calls to shutdown and causing
        // all waiters to immediately return in the future
        if self
            .inner
            .is_shutdown
            .swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            panic!("cannot call shutdown more than once");
        }

        // Release all pending shutdown waiters
        self.inner.notify.notify_waiters();

        // Wait for all extra strong references to the shared inner to drop.
        // There are two accounted for references, one in the controller,
        // and one waiter that the provider uses as a dependency..
        let mut count;
        while {
            count = Arc::count(&self.inner);
            count > 2
        } {
            trace!("Waiting for {} shutdown waiter(s) to drop", count - 2);
            self.rx
                .recv()
                .await
                .expect("failed to wait for next waiter drop signal");
        }
    }
}

/// Waiter utility for shutdown
#[derive(Clone)]
pub struct ShutdownWaiter(Arc<ShutdownInner>);

impl ShutdownWaiter {
    #[inline(always)]
    pub fn is_shutdown(&self) -> bool {
        self.0.is_shutdown.load(Ordering::Relaxed)
    }

    #[inline]
    pub async fn fold_until_shutdown<F, O, T, R>(&self, init: T, mut task: F) -> Option<R>
    where
        F: FnMut(T) -> O,
        O: Future<Output = ControlFlow<R, T>>,
    {
        if self.is_shutdown() {
            return None;
        }

        let mut notified = Box::pin(self.0.notify.notified());
        let mut current = init;

        loop {
            let output = Box::pin(task(current));
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

    /// Standalone function to wait until the shutdown signal is received.
    /// This function is 100% cancel safe and will always return immediately
    /// if shutdown has already happened.
    pub async fn wait_for_shutdown(&self) {
        if self.is_shutdown() {
            // There was a missed notify event, so we immediately return
            return;
        }

        self.0.notify.notified().await
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
        tokio::select! {
            biased;
            _ = self.wait_for_shutdown() => None,
            res = fut => Some(res)
        }
    }
}

impl Drop for ShutdownWaiter {
    fn drop(&mut self) {
        // Send drop signal only if shutdown has been triggered
        if self
            .0
            .is_shutdown
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            self.0.tx.send(()).ok();
        }
    }
}

/// Any object that implements the cryptographic digest function, this should
/// use a collision resistant hash function and have a representation agnostic
/// hashing for our core objects. Re-exported from [`ink_quill`]
pub use ink_quill::ToDigest;
pub use ink_quill::TranscriptBuilder;
