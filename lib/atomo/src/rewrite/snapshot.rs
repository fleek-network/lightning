use std::{
    ptr::null_mut,
    sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};

use fxhash::FxHashMap;
use seize::{reclaim, Collector, Linked};

use crate::once_ptr::OncePtr;

/// The snapshot list data structure is used to maintain a concurrent and garbage-collected
/// linked-list of snapshots.
///
/// The idea of this data structure comes from the fact that the first snapshot only contains
/// a delta and points directly to the persistence layer.
pub struct SnapshotList {
    collector: Collector,
    head: AtomicPtr<Linked<SnapshotInner>>,
    tail: AtomicPtr<Linked<SnapshotInner>>,
}

impl SnapshotList {
    /// Create a new empty snapshot list.
    pub fn new() -> Self {
        let collector = Collector::new();
        let node = collector.link_boxed(SnapshotInner::empty());

        SnapshotList {
            collector,
            head: AtomicPtr::new(node),
            tail: AtomicPtr::new(node),
        }
    }

    /// # Safety
    ///
    /// This function can not be called concurrently. Only one thread can access it at a time.
    pub fn push<F>(&self, batch: Batch, transition: F)
    where
        F: FnOnce(),
    {
        let guard = self.collector.enter();

        // Create the new head.
        let new_head = self.collector.link_boxed(SnapshotInner::empty());

        // Update the previous head's inner data.
        let prev_head_ptr = guard.protect(&self.head, Ordering::Acquire);
        let prev_head = unsafe { &*prev_head_ptr };
        prev_head.data.store(batch);
        prev_head.next.store(new_head, Ordering::Relaxed);

        // Perform the transition function that should be called in between.
        transition();

        // Set the new head.
        self.head.store(new_head, Ordering::Relaxed);

        // Now let's retire the oldest snapshots.
        let mut tail = self.tail.load(Ordering::Relaxed);
        let original_tail = tail;

        loop {
            if tail == prev_head_ptr {
                break;
            }

            let tail_ref = unsafe { &*tail };
            let counter = tail_ref.counter.load(Ordering::Relaxed);

            if counter > 0 {
                // there is at least one [`Snapshot`] that is still around.
                break;
            }

            println!("retire {:?}", tail_ref.data.load().map(|t| *t.0));
            unsafe {
                self.collector.retire(tail, reclaim::boxed::<SnapshotInner>);
            }

            // move to the next node and loop again.
            tail = tail_ref.next.load(Ordering::Relaxed);
        }

        if tail != original_tail {
            self.tail.store(tail, Ordering::Relaxed);
        }
    }

    /// Returns the current snapshot for the data.
    pub fn current(&self) -> Snapshot {
        let guard = self.collector.enter();
        let head_ptr = guard.protect(&self.head, Ordering::Acquire);
        Snapshot::new(head_ptr)
    }
}

pub struct Snapshot {
    inner: *mut Linked<SnapshotInner>,
}

struct SnapshotInner {
    counter: AtomicUsize,
    next: AtomicPtr<Linked<SnapshotInner>>,
    data: OncePtr<Batch>,
}

impl SnapshotInner {
    pub fn empty() -> Self {
        SnapshotInner {
            counter: AtomicUsize::new(0),
            next: AtomicPtr::new(null_mut()),
            data: OncePtr::null(),
        }
    }
}

impl Snapshot {
    fn new(inner_ptr: *mut Linked<SnapshotInner>) -> Self {
        let inner = unsafe { &*inner_ptr };
        let x = inner.counter.fetch_add(1, Ordering::Relaxed);
        Self { inner: inner_ptr }
    }

    fn get(&self) -> Option<usize> {
        unsafe { &*self.inner }.data.load().map(|b| *b.0)
    }
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        let inner = unsafe { &*self.inner };
        inner.counter.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Drop for SnapshotList {
    fn drop(&mut self) {
        let mut tail = self.tail.load(Ordering::Relaxed);

        loop {
            if tail.is_null() {
                break;
            }

            let tail_ref = unsafe { &*tail };

            println!("retire on drop {:?}", tail_ref.data.load().map(|t| *t.0));
            unsafe {
                self.collector.retire(tail, reclaim::boxed::<SnapshotInner>);
            }

            // move to the next node and loop again.
            tail = tail_ref.next.load(Ordering::Relaxed);
        }
    }
}

type BoxedVec = Box<[u8]>;

pub struct Batch(Box<usize>);

impl Drop for Batch {
    fn drop(&mut self) {
        println!("drop {}", *self.0);
    }
}

#[test]
fn rewrite() {
    use std::{sync::Arc, time::Duration};

    let list = Arc::new(SnapshotList::new());
    let handles: Vec<_> = (0..8)
        .map(|_| list.clone())
        .map(|list| {
            std::thread::spawn(move || {
                for _ in 0..5_000 {
                    let snapshot = list.current();
                    std::thread::sleep(Duration::from_micros(750));
                }
            })
        })
        .collect();

    for i in 0..1_000 {
        println!("--- {i}");

        list.push(Batch(Box::new(i)), || {});
        std::thread::sleep(Duration::from_micros(1000));
    }

    handles
        .into_iter()
        .for_each(|handle| handle.join().unwrap());

    println!("--------------- END ----------------");
}
