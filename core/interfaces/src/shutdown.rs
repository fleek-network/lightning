use std::future::Future;
use std::ops::ControlFlow;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::Poll;
use std::time::Duration;

pub use fdi::{Cloned, Consume, Ref, RefMut};
use futures::future::{select, Either};
use tokio::sync::futures::Notified;
use tokio::sync::Notify;
use tokio::time::timeout;
use tracing::trace;
use triomphe::Arc;

/// The shared state between [ShutdownController] and [ShutdownWaiter].
#[derive(Default)]
struct ShutdownInner {
    /// Used to notify the [ShutdownFuture]s when it is time to shutdown.
    notify_shutdown: Notify,

    /// Notified by [ShutdownFuture] drop when the pending counter reaches zero.
    notify_all_futures_droped: Notify,

    /// The high bit indicates if we have already shutdown or not the rest of the bits counts the
    /// number of pending shutdown futures.
    ///
    /// This allows a shutdown future to use one atomic operation (`fetch_add`) to both increment
    /// the pending counter as well as getting the previous number which would be useful to see
    /// if we have already shutdown or not.
    state: AtomicU64,
}

impl ShutdownInner {
    /// Set the high bit of the atomic state. Returns the previous number.
    fn set_shutdown_bit(&self) -> u64 {
        self.state.fetch_or(1 << 63, Ordering::SeqCst)
    }

    /// Load and unpack the atomic state variable into a boolean indicating if we have shutdown
    /// or not and the number of pending futures.
    #[inline(always)]
    fn unpack(&self, order: Ordering) -> (bool, u64) {
        let v = self.state.load(order);
        (is_msb_set(v), clear_msb(v))
    }
}

/// Controller utility for shutdown
#[derive(Default)]
pub struct ShutdownController {
    inner: Arc<ShutdownInner>,
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

    /// Trigger the shutdown without waiting for the pending futures to drop.
    pub fn trigger_shutdown(&self) {
        // Set the high bit of the atomic state to 1. Any future created after this point in
        // the code is actually not awaiting our shutdown signal and already start in the
        // is_shutdown=true state.
        let prev_state = self.inner.set_shutdown_bit();

        if is_msb_set(prev_state) {
            panic!("cannot call shutdown more than once");
        }

        // In case there are no futures pending already return early.
        if clear_msb(prev_state) == 0 {
            return;
        }

        // Release all pending shutdown waiters
        self.inner.notify_shutdown.notify_waiters(); // To wake up all the current tasks.
        self.inner.notify_shutdown.notify_one(); // To store a permit for future.
    }

    /// Trigger the shutdown signal and wait for all the child [`ShutdownWaiter`]'s are dropped.
    /// This function WILL hang if any waiters are not correctly dropped on shutdown.
    pub async fn shutdown(&self) {
        // Register the waiter before sending the shutdown so we won't lose any of its signals.
        let mut drop_notified = Box::pin(self.inner.notify_all_futures_droped.notified());

        self.trigger_shutdown();

        // From this point forward we're simply waiting for the number of pending futures
        // to reach zero.
        //
        // The timeout will allow us to print the trace every 3 seconds.
        loop {
            let pending_futures = self.inner.unpack(Ordering::Relaxed).1;
            if pending_futures == 0 {
                return;
            }
            trace!("Waiting for {pending_futures} shutdown waiter(s) to drop");
            let _ = timeout(Duration::from_secs(3), &mut drop_notified).await;
        }
    }
}

/// The waiter end of a [ShutdownController].
#[derive(Clone)]
pub struct ShutdownWaiter(Arc<ShutdownInner>);

impl ShutdownWaiter {
    /// Returns true if shutdown has already been called.
    #[inline(always)]
    pub fn is_shutdown(&self) -> bool {
        self.0.unpack(Ordering::Relaxed).0
    }

    /// Standalone function to wait until the shutdown signal is received.
    /// This function is 100% cancel safe and will always return immediately
    /// if shutdown has already happened.
    #[inline(always)]
    pub fn wait_for_shutdown(&self) -> ShutdownFuture<'_> {
        ShutdownFuture::new(self)
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

    #[inline]
    pub async fn fold_until_shutdown<F, O, T, R>(&self, init: T, mut task: F) -> Option<R>
    where
        F: FnMut(T) -> O,
        O: Future<Output = ControlFlow<R, T>>,
    {
        if self.is_shutdown() {
            return None;
        }

        let mut notified = Box::pin(self.wait_for_shutdown());
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
}

/// The future object that awaits the shutdown. This future is fused and once it returns
/// [`Poll::Ready`] it will always return ready on all of the next calls.
///
/// This type is to be considered structurally pinned.
pub struct ShutdownFuture<'a> {
    /// The shutdown waiter that produced this future.
    waiter: &'a ShutdownWaiter,

    /// Indicates if we have already shutdown. This makes `poll` not require atomic loads.
    is_shutdown: bool,

    /// The result of `waiter.notify.notified()`
    // TODO(qti3e): This can be combined with is_shutdown into an `Option<_>`.
    notified: Notified<'a>,
}

impl<'a> ShutdownFuture<'a> {
    #[inline(always)]
    fn new(waiter: &'a ShutdownWaiter) -> Self {
        let notified = waiter.0.notify_shutdown.notified();
        let value = waiter.0.state.fetch_add(1, Ordering::SeqCst);
        let is_shutdown = is_msb_set(value);

        Self {
            waiter,
            is_shutdown,
            notified,
        }
    }

    /// Project a pinned [ShutdownFuture] into a pinned [Notified] and provide mutable access to the
    /// [Self::is_shutdown] field all in the same call.
    #[inline(always)]
    fn unzip(self: Pin<&mut Self>) -> (Pin<&mut Notified<'a>>, &mut bool) {
        // SAFETY: the caller is responsible for not moving the
        // value out of this reference.
        let pointer = unsafe { Pin::get_unchecked_mut(self) };
        let notified_pointer = &mut pointer.notified;
        // SAFETY: This is okay because this field is never considered to be pinned anywhere
        // else.
        let is_shutdown_pointer = &mut pointer.is_shutdown;
        (
            // SAFETY: as the value of `this` is guaranteed to not have
            // been moved out, this call to `new_unchecked` is safe.
            unsafe { Pin::new_unchecked(notified_pointer) },
            is_shutdown_pointer,
        )
    }
}

impl<'a> Future for ShutdownFuture<'a> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        if self.is_shutdown {
            return Poll::Ready(());
        }

        let (notified, is_shutdown) = ShutdownFuture::unzip(self);

        match Future::poll(notified, cx) {
            Poll::Ready(_) => {
                *is_shutdown = true;
                Poll::Ready(())
            },
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<'a> Drop for ShutdownFuture<'a> {
    // SAFETY: We do not move any of the subfields in this function which meets the requirements
    // stated by [`Pin`].
    fn drop(&mut self) {
        // Load the state atomic, clear the high bit and if the previous value was 1 this means
        // we are currently at zero and that it is time to wake up the ctrl.
        if clear_msb(self.waiter.0.state.fetch_sub(1, Ordering::SeqCst)) == 1 {
            // We use `notify_waiters` here so that no permit is stored.
            self.waiter.0.notify_all_futures_droped.notify_waiters();
        }
    }
}

/// Given a u64 as input return another number but with the most significant bit set to zero.
#[inline(always)]
fn clear_msb(v: u64) -> u64 {
    v & !(1 << 63)
}

/// Given a u64 number returns true if the most significant bit is set.
#[inline(always)]
fn is_msb_set(v: u64) -> bool {
    (v >> 63) == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msb_util() {
        assert!(!is_msb_set(0));
        assert!(!is_msb_set(1));
        assert!(is_msb_set(u64::MAX));
        assert!(is_msb_set(1 << 63));
        assert!(is_msb_set((1 << 63) + 2));
        assert!(is_msb_set((1 << 63) + 100));
        assert_eq!(clear_msb((1 << 63) + 100), 100);
        assert_eq!(clear_msb((1 << 63) + 27), 27);
        assert_eq!(clear_msb(100), 100);
        assert_eq!(clear_msb(27), 27);
        assert_eq!(clear_msb(0), 0);
    }

    #[test]
    fn drop_should_update_counter_alive() {
        let ctrl = ShutdownController::new();
        let waiter = ctrl.waiter();
        assert_eq!(ctrl.inner.unpack(Ordering::Relaxed), (false, 0));
        let fut1 = waiter.wait_for_shutdown();
        assert_eq!(ctrl.inner.unpack(Ordering::Relaxed), (false, 1));
        let fut2 = waiter.wait_for_shutdown();
        assert_eq!(ctrl.inner.unpack(Ordering::Relaxed), (false, 2));
        drop(fut1);
        assert_eq!(ctrl.inner.unpack(Ordering::Relaxed), (false, 1));
        drop(fut2);
        assert_eq!(ctrl.inner.unpack(Ordering::Relaxed), (false, 0));
    }

    #[test]
    fn drop_should_update_counter_shutdown() {
        let ctrl = ShutdownController::new();
        let waiter = ctrl.waiter();
        assert_eq!(ctrl.inner.unpack(Ordering::Relaxed), (false, 0));
        let fut1 = waiter.wait_for_shutdown();
        assert_eq!(ctrl.inner.unpack(Ordering::Relaxed), (false, 1));
        let fut2 = waiter.wait_for_shutdown();
        assert_eq!(ctrl.inner.unpack(Ordering::Relaxed), (false, 2));
        ctrl.trigger_shutdown();
        drop((fut1, fut2));
        assert_eq!(ctrl.inner.unpack(Ordering::Relaxed), (true, 0));
    }

    #[tokio::test]
    async fn shutdown_should_notify_futures() {
        let ctrl = ShutdownController::new();
        let w1 = ctrl.waiter();
        let w2 = ctrl.waiter();
        tokio::spawn(async move {
            w1.wait_for_shutdown().await;
        });
        tokio::spawn(async move {
            w2.wait_for_shutdown().await;
        });
        assert!(
            timeout(Duration::from_millis(50), ctrl.shutdown())
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn shutdown_should_hang_with_pending_future() {
        let ctrl = ShutdownController::new();
        let w1 = ctrl.waiter();
        let w2 = ctrl.waiter();
        tokio::spawn(async move {
            w1.wait_for_shutdown().await;
        });
        let fut2 = w2.wait_for_shutdown();
        assert!(
            timeout(Duration::from_millis(50), ctrl.shutdown())
                .await
                .is_err()
        );
        assert_eq!(ctrl.inner.unpack(Ordering::Relaxed), (true, 1));
        drop(fut2);
        assert_eq!(ctrl.inner.unpack(Ordering::Relaxed), (true, 0));
    }

    #[tokio::test]
    async fn futures_drop_should_notify_ctrl() {
        let ctrl = ShutdownController::new();
        let waiter = ctrl.waiter();
        let fut1 = waiter.wait_for_shutdown();
        let fut2 = waiter.wait_for_shutdown();
        let mut notified = Box::pin(ctrl.inner.notify_all_futures_droped.notified());
        assert_eq!(ctrl.inner.unpack(Ordering::Relaxed), (false, 2));
        drop(fut1);
        assert!(
            timeout(Duration::from_millis(50), &mut notified)
                .await
                .is_err()
        );
        drop(fut2);
        assert_eq!(ctrl.inner.unpack(Ordering::Relaxed), (false, 0));
        assert!(
            timeout(Duration::from_millis(50), &mut notified)
                .await
                .is_ok()
        );
    }
}
