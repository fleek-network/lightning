use std::pin::Pin;
use std::task::Poll;

use futures::Future;
use triomphe::Arc;

use crate::shared::SharedState;
use crate::wait_list::WaitListSlotPos;

/// Owned version of [ShutdownSignal](crate::ShutdownSignal) future. It can be created `from` a
/// [ShutdownWaiter](crate::ShutdownWaiter).
pub struct OwnedShutdownSignal {
    /// Our reference to the state shared between all of the waiters/futures under the same
    /// shutdown controller.
    state: Arc<SharedState>,

    /// The wait list shard this signal belongs to.
    shard: usize,

    /// Our position in the wait list.
    list_position: Option<WaitListSlotPos>,
}

impl OwnedShutdownSignal {
    #[inline]
    pub(crate) fn new(state: Arc<SharedState>, shard: usize) -> Self {
        Self {
            state,
            shard,
            list_position: None,
        }
    }
}

impl Future for OwnedShutdownSignal {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut wait_list = this.state.wait_list_shards[this.shard].lock().unwrap();
        wait_list.poll(&mut this.list_position, cx)
    }
}

impl Drop for OwnedShutdownSignal {
    fn drop(&mut self) {
        if let Some(slot) = self.list_position.take() {
            let mut wait_list = self.state.wait_list_shards[self.shard]
                .lock()
                .expect("wait_list lock poisoned");
            wait_list.deregister(slot);
            if wait_list.is_done() {
                self.state.decrement_pending_wait_list_count();
            }
        }
    }
}
