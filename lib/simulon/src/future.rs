use std::sync::{Arc, Mutex, Weak};

use futures::Future;
use futures_task::{Poll, Waker};

/// A future that we can wake up from anywhere.
pub struct DeferredFuture<T>(Arc<Inner<T>>);

/// The waker of a [`DeferredFuture`].
pub struct DeferredFutureWaker<T>(Weak<Inner<T>>);

struct Inner<T>(Mutex<(Option<T>, Option<Waker>)>);

impl<T> DeferredFuture<T> {
    pub fn new() -> Self {
        DeferredFuture(Arc::new(Inner(Mutex::new((None, None)))))
    }

    pub fn resolved(value: T) -> Self {
        DeferredFuture(Arc::new(Inner(Mutex::new((Some(value), None)))))
    }

    pub fn waker(&self) -> DeferredFutureWaker<T> {
        DeferredFutureWaker(Arc::downgrade(&self.0))
    }
}

impl<T> Future for DeferredFuture<T> {
    type Output = T;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut futures_task::Context<'_>,
    ) -> futures_task::Poll<Self::Output> {
        let mut guard = self.0.0.lock().unwrap();

        if let Some(value) = guard.0.take() {
            Poll::Ready(value)
        } else {
            guard.1 = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl<T> DeferredFutureWaker<T> {
    /// Wake up the future.
    pub fn wake(&self, value: T) {
        if let Some(inner) = self.0.upgrade() {
            let mut guard = inner.0.lock().unwrap();
            guard.0 = Some(value);
            if let Some(waker) = guard.1.take() {
                waker.wake();
            }
        }
    }
}
