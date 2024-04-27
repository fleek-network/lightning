use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

use arrayvec::ArrayVec;

use crate::wait_list::WaitList;

/// The data shared between shutdown controller and the waiters.
pub(crate) struct SharedState {
    pub pending_waiting_lists: AtomicUsize,

    pub wait_list_shards: ArrayVec<Mutex<WaitList>, 32>,

    pub waiting_for_drop: Mutex<WaitList>,

    pub next_shard: AtomicU64,
}

unsafe impl Send for SharedState {}
unsafe impl Sync for SharedState {}

impl SharedState {
    pub fn new(shards: usize) -> Self {
        let mut wait_list_shards = ArrayVec::new();
        for _ in 0..shards {
            wait_list_shards.push(Mutex::new(WaitList::with_capacity((2048 / shards).max(8))));
        }

        Self {
            pending_waiting_lists: AtomicUsize::new(0),
            wait_list_shards,
            waiting_for_drop: Mutex::new(WaitList::with_capacity(8)),
            next_shard: AtomicU64::new(0),
        }
    }

    pub fn get_shard(&self) -> usize {
        let len = self.wait_list_shards.len() as u64;
        if len == 1 {
            0
        } else {
            (self
                .next_shard
                .fetch_add(1, Ordering::Relaxed)
                .wrapping_mul(len)
                >> 32) as u32 as usize
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
}
