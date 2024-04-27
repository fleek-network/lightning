use std::sync::atomic::Ordering;

use triomphe::Arc;

use crate::completion_fut::CompletionFuture;
use crate::shared::SharedState;
use crate::signal_fut::ShutdownSignal;
use crate::OwnedShutdownSignal;

pub struct ShutdownController {
    inner: Arc<SharedState>,
}

pub struct ShutdownWaiter {
    inner: Arc<SharedState>,
}

impl ShutdownController {
    /// Create a new shutdown controller with the given number of wait list shards.
    pub fn new(shards: usize) -> Self {
        ShutdownController {
            inner: Arc::new(SharedState::new(shards)),
        }
    }

    /// Returns the waiter end of this [ShutdownController]. A waiter can be used to create many
    /// futures awaiting the shutdown.
    pub fn waiter(&self) -> ShutdownWaiter {
        ShutdownWaiter {
            inner: self.inner.clone(),
        }
    }

    pub fn trigger_shutdown(&self) {
        /// To trigger the shutdown we first close every single shard. This would result in each
        /// of the closed wait lists to not have any more new registerations and therefore it
        /// 'locks'/'freezes' the number of wakers in each wait list.
        ///
        /// We count the number of the wait lists that have at least one waker depending on them
        /// to wake up. When the last future (that was part of the list = had an active
        /// registration) is dropped it will decrement a one from this atomic and when it reaches
        /// zero it will be responsible to wake up the [SharedState::waiting_for_drop] waker.
        ///
        /// If if happens that the number of non-empty wait lists are zero initially, we wake up
        /// that future in this same function.
        fn _docs() {} // just so we can use `///` so the links in [] can work!

        // Shard index of the non-empty wait lists.
        let mut alive_wait_lists = Vec::with_capacity(self.inner.wait_list_shards.len());

        for (i, wait_list_mutex) in self.inner.wait_list_shards.iter().enumerate() {
            let mut wait_list = wait_list_mutex.lock().unwrap();
            wait_list.close();

            if !wait_list.is_empty() {
                alive_wait_lists.push(i);
            }
        }

        self.inner
            .pending_waiting_lists
            .store(alive_wait_lists.len(), Ordering::SeqCst);

        // If there are no
        if alive_wait_lists.is_empty() {
            self.inner.waiting_for_drop.lock().unwrap().wake_all();
        }

        for wait_list_mutex in alive_wait_lists
            .into_iter()
            .map(|i| &self.inner.wait_list_shards[i])
        {
            let mut wait_list = wait_list_mutex.lock().unwrap();
            wait_list.wake_all();
        }
    }

    /// Returns a future that is resolved as soon as all of the futures waiting for shutdown
    /// have dropped.
    pub fn wait_for_completion(&self) -> CompletionFuture {
        CompletionFuture::new(&self.inner)
    }
}

impl ShutdownWaiter {
    pub fn wait_for_shutdown(&self, i: u8) -> ShutdownSignal {
        ShutdownSignal::new(&self.inner, i as usize)
    }
}

impl From<ShutdownWaiter> for OwnedShutdownSignal {
    fn from(value: ShutdownWaiter) -> Self {
        OwnedShutdownSignal::new(value.inner, 0)
    }
}
