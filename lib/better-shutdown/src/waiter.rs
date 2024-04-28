use std::sync::Mutex;

use triomphe::Arc;

use crate::shared::SharedState;
use crate::wait_list::WaitList;
use crate::ShutdownSignal;

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

    /// Create a future that will be resolved when shutdown arrives.
    pub fn wait_for_shutdown(&self, _: u8) -> ShutdownSignal {
        ShutdownSignal::new(&self.inner, self.dedicated.as_deref())
    }
}
