use std::pin::Pin;
use std::sync::Mutex;
use std::task::Poll;

use futures::Future;

use crate::shared::{SharedState, NUM_SHARED_SHARDS};
use crate::wait_list::{WaitList, WaitListSlotPos};

/// A future that is resolved once the shutdown is triggered. It has the life time of the
/// waiter it was created from.
///
/// There is also an owned version.
pub struct ShutdownSignal<'a> {
    /// Our reference to the state shared between all of the waiters/futures under the same
    /// shutdown controller.
    state: &'a SharedState,

    /// The wait list shard this signal belongs to.
    shard: Option<&'a Mutex<WaitList>>,

    /// Our position in the wait list.
    list_position: Option<WaitListSlotPos>,
}

impl<'a> ShutdownSignal<'a> {
    #[inline]
    pub(crate) fn new(state: &'a SharedState, shard: Option<&'a Mutex<WaitList>>) -> Self {
        Self {
            state,
            shard,
            list_position: None,
        }
    }
}

impl<'a> Future for ShutdownSignal<'a> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        let ptr = this as *const _ as *const () as usize as u64;
        let shard = *this.shard.get_or_insert_with(|| {
            let out = fxhash::hash32(&ptr) % (NUM_SHARED_SHARDS as u32);
            &this.state.wait_list_shards[out as usize]
        });

        let mut wait_list = shard.lock().unwrap();
        wait_list.poll(&mut this.list_position, cx)
    }
}

impl<'a> Drop for ShutdownSignal<'a> {
    fn drop(&mut self) {
        if let Some(slot) = self.list_position.take() {
            let mut wait_list = self.shard.unwrap().lock().expect("wait_list lock poisoned");
            wait_list.deregister(slot);
            if wait_list.is_done() {
                self.state.decrement_pending_wait_list_count();
            }
        }
    }
}
