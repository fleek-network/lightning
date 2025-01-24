use std::pin::Pin;
use std::task::Poll;

use futures::Future;

use crate::shared::SharedState;
use crate::wait_list::WaitListSlotPos;

/// A future that is resolved once all of the [ShutdownSignal](crate::ShutdownSignal) have dropped.
pub struct CompletionFuture<'a> {
    state: &'a SharedState,

    /// Our position in the wait list.
    list_position: Option<WaitListSlotPos>,
}

impl<'a> CompletionFuture<'a> {
    #[inline]
    pub(crate) fn new(state: &'a SharedState) -> Self {
        Self {
            state,
            list_position: None,
        }
    }
}

impl Future for CompletionFuture<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut wait_list = this.state.waiting_for_drop.lock().unwrap();
        wait_list.poll(&mut this.list_position, cx)
    }
}

impl Drop for CompletionFuture<'_> {
    fn drop(&mut self) {
        if let Some(slot) = self.list_position.take() {
            let mut wait_list = self
                .state
                .waiting_for_drop
                .lock()
                .expect("wait_list lock poisoned");
            wait_list.deregister(slot);
        }
    }
}
