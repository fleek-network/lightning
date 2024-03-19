use futures::Future;
use futures_task::{Poll, Waker};
use parking_lot::Mutex;
use triomphe::Arc;

/// A future that we can wake up from anywhere.
pub struct DeferredFuture<T>(Arc<Mutex<Inner<T>>>);

/// The waker of a [`DeferredFuture`].
pub struct DeferredFutureWaker<T>(Arc<Mutex<Inner<T>>>);

struct Inner<T> {
    value: Option<T>,
    waker: Option<Waker>,
}

impl<T> DeferredFuture<T> {
    #[inline(always)]
    pub fn new() -> Self {
        DeferredFuture(Arc::new(Mutex::new(Inner {
            value: None,
            waker: None,
        })))
    }

    #[inline(always)]
    pub fn resolved(value: T) -> Self {
        DeferredFuture(Arc::new(Mutex::new(Inner {
            value: Some(value),
            waker: None,
        })))
    }

    #[inline(always)]
    pub fn waker(&self) -> DeferredFutureWaker<T> {
        DeferredFutureWaker(self.0.clone())
    }
}

impl<T> Future for DeferredFuture<T> {
    type Output = T;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut futures_task::Context<'_>,
    ) -> futures_task::Poll<Self::Output> {
        let mut guard = self.0.lock();

        if let Some(value) = guard.value.take() {
            Poll::Ready(value)
        } else {
            guard.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl<T> DeferredFutureWaker<T> {
    /// Wake up the future.
    #[inline(always)]
    pub fn wake(&self, value: T) {
        let mut guard = self.0.lock();
        guard.value = Some(value);
        if let Some(waker) = guard.waker.take() {
            waker.wake();
        }
    }
}
