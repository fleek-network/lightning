use std::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, Ordering};

use futures::future::{select, Either};
use futures::Future;
use tokio::sync::Notify;
use triomphe::Arc;

#[derive(Default)]
pub struct ShutdownNotifier {
    inner: Arc<Inner>,
}

impl Drop for ShutdownNotifier {
    fn drop(&mut self) {
        self.inner.is_shutdown.store(true, Ordering::Relaxed);
        self.inner.notify.notify_waiters();
    }
}

#[derive(Clone)]
pub struct ShutdownWaiter {
    inner: Arc<Inner>,
}

#[derive(Default)]
struct Inner {
    notify: Notify,
    is_shutdown: AtomicBool,
}

impl ShutdownNotifier {
    pub fn shutdown(self) {
        self.inner.is_shutdown.store(true, Ordering::Relaxed);
        self.inner.notify.notify_waiters();
    }

    #[inline]
    pub fn waiter(&self) -> ShutdownWaiter {
        ShutdownWaiter {
            inner: self.inner.clone(),
        }
    }
}

impl ShutdownWaiter {
    pub fn is_shutdown(&self) -> bool {
        self.inner.is_shutdown.load(Ordering::Relaxed)
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

        let mut notified = Box::pin(self.inner.notify.notified());
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
    pub async fn run_until_shutdown<F, O, R>(&self, mut task: F) -> Option<R>
    where
        F: FnMut() -> O,
        O: Future<Output = ControlFlow<R>>,
    {
        self.fold_until_shutdown((), |_| task()).await
    }
}
