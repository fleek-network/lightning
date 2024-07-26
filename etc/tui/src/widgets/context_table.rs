use crate::components::Extractor;
use crate::components::DynExtractor;
use crate::helpers::StableVec;

use ratatui::widgets::TableState;

pub struct Table<C, T> {
    values: DynExtractor<C, StableVec<T>>,
    removing: Vec<T>,
    last_known_len: Option<usize>,
    name: &'static str,
    state: TableState,
}

impl<C, T> Table<C, T> {
    pub fn new<E>(name: &'static str, vals: E) -> Self 
        where E: Extractor<C, StableVec<T> > + 'static
    {
        Self {
            values: Box::new(vals),
            removing: Vec::new(),
            last_known_len: None,
            name,
            state: TableState::default().with_selected(None),
        }
    }

    pub fn records<'a>(&self, context: &'a mut C) -> &'a Vec<T> {
        self.values.get(context)
    }

    pub fn scroll_up(&mut self) {
        if let Some(cur) = self.state.selected() {
            if cur > 0 {
                let cur = cur - 1;
                self.state.select(Some(cur));
            }
        }
    }

    pub fn scroll_down(&mut self, context: &mut C) {
        if let Some(cur) = self.state.selected() {
            let len = self.values.get(context).len();
            if len > 0 && cur < len - 1 {
                let cur = cur + 1;
                self.state.select(Some(cur));
            }
        }
    }

    pub fn remove_selected_record(&mut self, context: &mut C) {
        if let Some(cur) = self.state.selected() {
            debug_assert!(cur < self.records(context).len());

            let values = unsafe { self.values.get(context).as_vec() };

            let removing = values.remove(cur);
            if let Some(len) = self.last_known_len {
                if cur < len - 1 {
                    self.last_known_len = if len == 1 { None } else { Some(len) };
                    self.removing.push(removing);
                }
            }

            if self.records(context).is_empty() {
                self.state.select(None);
            } else if cur == self.records(context).len() {
                self.state.select(Some(cur - 1));
            } else {
                self.state.select(Some(cur));
            }
        }
    }

    pub fn restore_state(&mut self, context: &mut C) {
        let buf = unsafe { self.values.get(context).as_vec() };
        if let Some(len) = self.last_known_len {
            buf.truncate(len);
        }

        buf.extend(self.removing.drain(..));

        if !buf.is_empty() {
            self.last_known_len = Some(buf.len());
            self.state.select(Some(0));
        } else {
            // The list should have been empty to start with.
            debug_assert!(self.last_known_len.is_none());
        }
    }

    /// Commit changes to the list and get all the removed values
    pub fn commit_changes(&mut self, context: &mut C) -> Vec<T> {
        self.last_known_len = Some(self.records(context).len());

        std::mem::take(&mut self.removing)
    }

    pub fn state(&mut self) -> &mut TableState {
        &mut self.state
    }
}