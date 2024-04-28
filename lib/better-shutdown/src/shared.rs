use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use triomphe::Arc;

use crate::wait_list::WaitList;

pub(crate) const NUM_SHARED_SHARDS: usize = 16;
pub(crate) const SHARED_SHARDS_TOTAL_DEFAULT_CAPACITY: usize = 2048;

/// The data shared between shutdown controller and the waiters.
pub(crate) struct SharedState {
    /// Useless before `trigger_shutdown` is called. But at that stage it is used to keep track
    /// of the number of 'frozen' wait lists that were not empty.
    pub pending_waiting_lists: AtomicUsize,

    /// A series of shared wait lists that are by default used by `ShutdownWaiter`'s that are not
    /// marked as dedicated.
    pub wait_list_shards: [Mutex<WaitList>; NUM_SHARED_SHARDS],

    /// The wait list used to notify us when all of the wait lists have completed.
    pub waiting_for_drop: Mutex<WaitList>,

    /// The dedicated wait lists are wait lists that are only used by one ShutdownWaiter (unless
    /// they are cloned).
    dedicated_wait_lists: Mutex<DedicatedWaitListState>,
}

struct DedicatedWaitListState {
    did_shutdown: bool,
    lists: Vec<Arc<Mutex<WaitList>>>,
}

impl SharedState {
    pub fn new(_: usize) -> Self {
        let wait_list_shards = std::array::from_fn(|_| {
            Mutex::new(WaitList::with_capacity(
                SHARED_SHARDS_TOTAL_DEFAULT_CAPACITY / NUM_SHARED_SHARDS,
            ))
        });

        Self {
            pending_waiting_lists: AtomicUsize::new(0),
            wait_list_shards,
            dedicated_wait_lists: Mutex::new(DedicatedWaitListState {
                did_shutdown: false,
                lists: Vec::with_capacity(16),
            }),
            waiting_for_drop: Mutex::new(WaitList::with_capacity(8)),
        }
    }

    /// Decrement the number of pending wait lists should be called when a wait list's last future
    /// is dropped and this potentially wakes up the `waiting_for_drop` wait list.
    #[inline]
    pub fn decrement_pending_wait_list_count(&self) {
        let prev = self.pending_waiting_lists.fetch_sub(1, Ordering::Relaxed);
        if prev == 1 {
            self.waiting_for_drop.lock().unwrap().wake_all();
        }
    }

    /// Create a new dedicated wait list. This function might return `None` if called during an
    /// active `trigger_shutdown` call.
    pub fn new_dedicated_wait_list(&self) -> Option<Arc<Mutex<WaitList>>> {
        let mut guard = self.dedicated_wait_lists.lock().unwrap();
        if guard.did_shutdown {
            None
        } else {
            let wait_list = Arc::new(Mutex::new(WaitList::default()));
            guard.lists.push(wait_list.clone());
            Some(wait_list)
        }
    }

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
    pub fn trigger_shutdown(&self) {
        // Shard index of the non-empty wait lists.
        let mut dedicated_list_guard = self.dedicated_wait_lists.lock().unwrap();
        let mut alive_wait_lists =
            Vec::with_capacity(NUM_SHARED_SHARDS + dedicated_list_guard.lists.len());

        // Tell the `new_dedicated_wait_list` to return `None` at this point.
        dedicated_list_guard.did_shutdown = true;

        for wait_list_mutex in self
            .wait_list_shards
            .iter()
            .chain(dedicated_list_guard.lists.iter().map(Arc::deref))
        {
            let mut wait_list = wait_list_mutex.lock().unwrap();
            wait_list.close();

            if !wait_list.is_empty() {
                alive_wait_lists.push(wait_list_mutex);
            }
        }

        self.pending_waiting_lists
            .store(alive_wait_lists.len(), Ordering::SeqCst);

        // If there are no alive wait lists no `Drop` is gonna notice this so it's up to us
        // here to wake up this tasks.
        if alive_wait_lists.is_empty() {
            self.waiting_for_drop.lock().unwrap().wake_all();
        }

        for wait_list_mutex in alive_wait_lists.into_iter() {
            let mut wait_list = wait_list_mutex.lock().unwrap();
            wait_list.wake_all();
        }
    }
}
