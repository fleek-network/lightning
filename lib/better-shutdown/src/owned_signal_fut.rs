use std::sync::Mutex;

use futures::Future;
use triomphe::Arc;

use crate::shared::{SharedState, NUM_SHARED_SHARDS};
use crate::wait_list::{WaitList, WaitListSlotPos};

/// An owned shutdown signal future.
pub struct OwnedShutdownSignal {
    /// The shared state which is used in order for drop to be able to notify that
    /// all waiters have dropped.
    state: Arc<SharedState>,

    /// Either an owned wait list for dedicated waiters or the shard index when in
    /// the shared mode.
    wait_list: OwnedWaitListRef,

    /// Our position in the wait list.
    list_position: Option<WaitListSlotPos>,
}

enum OwnedWaitListRef {
    DedicatedWaitList(Arc<Mutex<WaitList>>),
    SharedWaitList(Option<usize>),
}

impl OwnedShutdownSignal {
    pub(crate) fn new(state: Arc<SharedState>, wait_list: Option<Arc<Mutex<WaitList>>>) -> Self {
        Self {
            wait_list: wait_list
                .map(OwnedWaitListRef::DedicatedWaitList)
                .unwrap_or(OwnedWaitListRef::SharedWaitList(None)),
            state,
            list_position: None,
        }
    }
}

impl Future for OwnedShutdownSignal {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();
        let ptr = this as *const _ as *const () as usize as u64;

        let wait_list_mutex: &Mutex<WaitList> = match &mut this.wait_list {
            OwnedWaitListRef::DedicatedWaitList(wait_list) => wait_list,
            OwnedWaitListRef::SharedWaitList(shard) => {
                let shard = *shard.get_or_insert_with(|| {
                    (fxhash::hash32(&ptr) % (NUM_SHARED_SHARDS as u32)) as usize
                });
                &this.state.wait_list_shards[shard]
            },
        };

        let mut wait_list = wait_list_mutex.lock().unwrap();
        wait_list.poll(&mut this.list_position, cx)
    }
}

impl Drop for OwnedShutdownSignal {
    fn drop(&mut self) {
        if let Some(slot) = self.list_position.take() {
            let wait_list_mutex: &Mutex<WaitList> = match &self.wait_list {
                OwnedWaitListRef::DedicatedWaitList(wait_list) => wait_list,
                OwnedWaitListRef::SharedWaitList(Some(shard)) => {
                    &self.state.wait_list_shards[*shard]
                },
                _ => unreachable!(),
            };

            let mut wait_list = wait_list_mutex.lock().expect("wait_list lock poisoned");
            wait_list.deregister(slot);
            if wait_list.is_done() {
                self.state.decrement_pending_wait_list_count();
            }
        }
    }
}
