use triomphe::Arc;

use crate::backtrace_list::BacktraceList;
use crate::completion_fut::CompletionFuture;
use crate::shared::SharedState;
use crate::waiter::ShutdownWaiter;
use crate::BacktraceListIter;

/// The main struct of this crate which can be used to produce many [ShutdownWaiter]s.
///
/// The controller is allowed to trigger the shutdown event which will in turn be recivied by all
/// of the shutdown futures linked to the same controller.
pub struct ShutdownController {
    inner: Arc<SharedState>,
    backtrace_list: BacktraceList,
}

impl Default for ShutdownController {
    fn default() -> Self {
        Self::new(false)
    }
}

impl ShutdownController {
    /// Create a new shutdown controller with the given number of wait list shards.
    pub fn new(capture_backtrace: bool) -> Self {
        ShutdownController {
            inner: Arc::new(SharedState::new(capture_backtrace)),
            backtrace_list: BacktraceList::default(),
        }
    }

    /// Returns the waiter end of this [ShutdownController]. A waiter can be used to create many
    /// futures awaiting the shutdown.
    pub fn waiter(&self) -> ShutdownWaiter {
        ShutdownWaiter::new(self.inner.clone())
    }

    /// Trigger the shutdown event and wake up all of the outstanding shutdown futures.
    ///
    /// This method should only be called once and once called the system is marked as shutdown and
    /// calling it more than one time has no effect.
    ///
    /// This method immediately returns and does not wait for the shutdown to complete.
    pub fn trigger_shutdown(&self) {
        self.inner.trigger_shutdown()
    }

    /// Returns a future that is resolved as soon as all of the futures waiting for shutdown
    /// have dropped.
    pub fn wait_for_completion(&self) -> CompletionFuture {
        CompletionFuture::new(&self.inner)
    }

    /// Returns an iterator over all of the currently pending backtraces. This is a very expensive
    /// operation.
    pub fn pending_backtraces(&mut self) -> Option<BacktraceListIter> {
        if !self.inner.capture_backtrace {
            return None;
        }

        self.inner
            .collect_pending_backtrace(&mut self.backtrace_list);

        Some(self.backtrace_list.iter())
    }
}
