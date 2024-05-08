use std::num::NonZeroUsize;
use std::task::{Context, Poll, Waker};

use crate::backtrace_list::BacktraceList;

pub const WAIT_LIST_DEFAULT_CAPACITY: usize = 256;

/// The wait list contains a list of wakers (and some potential attached attributes) and can wake
/// all of these wakers up at any time.
///
/// But once it is closed it can not accept any more new waiters. Think of it as something similar
/// to the oneshot channel but for many receivers.
///
/// Closing the [WaitList] is exposed as a separate action than waking the tasks up. So that
/// two-phase close all and wake-up can be possible.
///
/// The current implementation uses an arena like structure but the same boundary APIs could be
/// used with an intrusive linked list in the back.
///
/// The arena structure has a vec of objects but anytime something is freed we replace it with
/// a link to the previously freed item. If the free list was empty we use [ArenaEntry::Empty]
/// instead.
pub struct WaitList {
    /// Set to true if we have to capture the backtrace of the poll functions.
    capture_backtrace: bool,

    /// The current state of wait list which begins at [State::Open] initially.
    state: State,

    /// Total number of actual items in the arena.
    len: usize,

    /// Position of the last freed item in the arena.
    last_freed: Option<usize>,

    /// The free-list embedded vector of items.
    arena: Vec<ArenaEntry>,
}

enum ArenaEntry {
    Empty,
    Link(usize),
    Item(WaitListEntry),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum State {
    Open,
    Closed,
    WokeUp,
}

/// Waker and its related attributes.
pub struct WaitListEntry {
    /// The waker that can be used to wake up a polled future.
    pub waker: Waker,

    /// The captured backtrace of the poll method on the waiter. If the feature is enabled.
    pub backtrace: Option<Box<std::backtrace::Backtrace>>,
}

/// An owning opaque pointer to a position in a wait list. It's simply a usize index that is not
/// Copy and Clone.
#[repr(transparent)]
pub struct WaitListSlotPos {
    // This allows us to use some memory layout optimizations when we put this in an Option.
    value: NonZeroUsize,
}

impl WaitListSlotPos {
    #[inline(always)]
    fn new(index: usize) -> Self {
        Self {
            // SAFETY: index >= 0 -> index + 1 >= 1 -> (index + 1) != 0
            value: unsafe { NonZeroUsize::new_unchecked(index + 1) },
        }
    }

    #[inline(always)]
    fn get(&self) -> usize {
        usize::from(self.value) - 1
    }
}

impl Default for WaitList {
    fn default() -> Self {
        Self::new(WAIT_LIST_DEFAULT_CAPACITY, false)
    }
}

impl WaitList {
    /// Create a new wait list with the provided capacity.
    pub fn new(capacity: usize, capture_backtrace: bool) -> Self {
        Self {
            capture_backtrace,
            state: State::Open,
            len: 0,
            last_freed: None,
            arena: Vec::with_capacity(capacity),
        }
    }

    /// Returns true if the wait list is empty.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline(always)]
    pub fn is_closed(&self) -> bool {
        !matches!(self.state, State::Open)
    }

    #[inline(always)]
    pub fn has_woke_up(&self) -> bool {
        matches!(self.state, State::WokeUp)
    }

    #[inline(always)]
    pub fn is_done(&self) -> bool {
        self.has_woke_up() && self.is_empty()
    }

    /// Register a new waker into the list. If `wake_all` has already been called this method
    /// fails and returns an error.
    #[inline]
    pub fn register(&mut self, e: WaitListEntry) -> Result<WaitListSlotPos, ()> {
        if self.is_closed() {
            return Err(());
        }

        let vec_len = self.arena.len();
        self.len += 1;

        // If we have enough capacity in the vec, don't bother consulting the free_slots list.
        if self.arena.capacity() == vec_len {
            if let Some(index) = self.last_freed {
                self.last_freed = match self.arena[index] {
                    ArenaEntry::Empty => None,
                    ArenaEntry::Link(index) => Some(index),
                    ArenaEntry::Item(_) => unreachable!(),
                };

                self.arena[index] = ArenaEntry::Item(e);
                return Ok(WaitListSlotPos::new(index));
            }
        }

        self.arena.push(ArenaEntry::Item(e));
        Ok(WaitListSlotPos::new(vec_len))
    }

    /// Remove a previously registered waker from the list.
    #[inline]
    pub fn deregister(&mut self, ptr: WaitListSlotPos) {
        self.len -= 1;
        self.arena[ptr.get()] = match self.last_freed {
            Some(index) => ArenaEntry::Link(index),
            None => ArenaEntry::Empty,
        };
        self.last_freed = Some(ptr.get());
    }

    /// Close the wait list making it not accept new registerations.
    #[inline]
    pub fn close(&mut self) {
        self.state = State::Closed;
    }

    /// Close the wait list and call the wake up on all of the registered waiters.
    pub fn wake_all(&mut self) {
        if self.has_woke_up() {
            return;
        }
        self.state = State::WokeUp;
        for item in &self.arena {
            if let ArenaEntry::Item(e) = item {
                e.waker.wake_by_ref();
            }
        }
    }

    /// Returns all of the captured backtraces.
    pub fn take_all_backtraces(&mut self, wait_list_index: usize, list: &mut BacktraceList) {
        if !self.capture_backtrace {
            return;
        }

        list.ensure_index_is_inserted(wait_list_index);
        list.reserve(wait_list_index, self.len);

        for (index, entry) in self.arena.iter_mut().enumerate() {
            match entry {
                ArenaEntry::Empty => {
                    list.remove(wait_list_index, index);
                },
                ArenaEntry::Link(_) => {
                    list.remove(wait_list_index, index);
                },
                ArenaEntry::Item(WaitListEntry { backtrace, .. }) => {
                    if let Some(trace) = backtrace.take() {
                        list.insert(wait_list_index, index, trace);
                    }
                },
            }
        }
    }

    /// The common implementation of poll shared by some of the futures. This is to be called
    /// by any future that wants to be part of a [WaitList]. The future has to maintain an
    /// state field [Option<WaitListSlotPos>] to keep track of its position in the wait list.
    #[inline(always)]
    pub fn poll(&mut self, prev_index: &mut Option<WaitListSlotPos>, cx: &mut Context) -> Poll<()> {
        if self.is_closed() {
            return Poll::Ready(());
        }

        let waker = cx.waker().clone();
        let backtrace = self
            .capture_backtrace
            .then(std::backtrace::Backtrace::force_capture)
            .map(Box::new);

        if let Some(idx) = &*prev_index {
            // As long as the index does exists the value at that position is not null and is valid
            // entry.
            match &mut self.arena[idx.get()] {
                ArenaEntry::Item(entry) => {
                    entry.waker = waker;
                    entry.backtrace = backtrace;
                },
                _ => panic!("wrong index!"),
            }
        } else {
            let idx = self.register(WaitListEntry { waker, backtrace }).unwrap();
            *prev_index = Some(idx);
        }

        Poll::Pending
    }
}
