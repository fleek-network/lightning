use std::backtrace::Backtrace;

use fxhash::FxHashMap;

#[derive(Default)]
pub struct BacktraceList {
    backtraces: Vec<FxHashMap<usize, Box<Backtrace>>>,
}

pub struct BacktraceListIter<'b> {
    list: &'b BacktraceList,
    index: usize,
    current_map_iter: Option<std::collections::hash_map::Iter<'b, usize, Box<Backtrace>>>,
}

impl BacktraceList {
    pub fn iter(&self) -> BacktraceListIter {
        BacktraceListIter::new(self)
    }

    #[inline(always)]
    pub fn ensure_index_is_inserted(&mut self, wait_list_index: usize) {
        while wait_list_index <= self.backtraces.len() {
            self.backtraces.push(FxHashMap::default());
        }
    }

    pub fn reserve(&mut self, wait_list_index: usize, size: usize) {
        let map = &mut self.backtraces[wait_list_index];
        if let Some(additional) = size.checked_sub(map.len()) {
            map.reserve(additional);
        }
    }

    pub fn insert(&mut self, wait_list_index: usize, index: usize, trace: Box<Backtrace>) {
        self.backtraces[wait_list_index].insert(index, trace);
    }

    pub fn remove(&mut self, wait_list_index: usize, index: usize) {
        self.backtraces[wait_list_index].remove(&index);
    }
}

/// An iterator over the backtrace of the pending shutdown tasks.
impl<'b> BacktraceListIter<'b> {
    pub fn new(list: &'b BacktraceList) -> Self {
        Self {
            list,
            index: 0,
            current_map_iter: None,
        }
    }
}

impl<'b> Iterator for BacktraceListIter<'b> {
    type Item = &'b Backtrace;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.index == self.list.backtraces.len() {
                return None;
            }

            let iter = self
                .current_map_iter
                .get_or_insert_with(|| self.list.backtraces[self.index].iter());

            match iter.next() {
                Some((_, trace_box)) => {
                    return Some(trace_box);
                },
                None => {
                    self.index += 1;
                    self.current_map_iter = None;
                },
            }
        }
    }
}
