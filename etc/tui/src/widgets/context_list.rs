use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::{Modifier, Style};
use ratatui::widgets::{Block, Borders, ListState};

use crate::components::{Extractor, DynExtractor};
pub struct ContextList<C, T> {
    values: DynExtractor<C, Vec<T>>,
    last_known_len: Option<usize>,
    removing: Vec<T>,
    name: &'static str,
    list_state: ListState,
}

impl<C, T> ContextList<C, T> {
    pub fn new<F>(vals: F, name: &'static str) -> Self 
    where
        F: Extractor<C, Vec<T>> + 'static,
    {
        Self {
            values: Box::new(vals),
            last_known_len: None,
            removing: Vec::new(),
            name,
            list_state: ListState::default().with_selected(None),
        }
    }

    pub fn records<'a>(&self, context: &'a mut C) -> &'a Vec<T> {
        self.values.get(context)
    }

    pub fn scroll_up(&mut self) {
        if let Some(cur) = self.list_state.selected() {
            if cur > 0 {
                let cur = cur - 1;
                self.list_state.select(Some(cur));
            }
        }
    }

    pub fn scroll_down(&mut self, context: &mut C) {
        if let Some(cur) = self.list_state.selected() {
            let len = self.records(context).len();
            if len > 0 && cur < len - 1 {
                let cur = cur + 1;
                self.list_state.select(Some(cur));
            }
        }
    }

    pub fn remove_selected_record(&mut self, context: &mut C) {
        if let Some(cur) = self.list_state.selected() {
            debug_assert!(cur < self.records(context).len());

            let removing = self.values.get(context).remove(cur);
            if let Some(len) = self.last_known_len {
                if cur < len - 1 {
                    self.last_known_len = if len == 1 { None } else { Some(len) };
                    self.removing.push(removing);
                }
            }
             
            if self.records(context).is_empty() {
                self.list_state.select(None);
            } else if cur == self.records(context).len() {
                self.list_state.select(Some(cur - 1));
            } else {
                self.list_state.select(Some(cur));
            }
        }
    }

    pub fn restore_state(&mut self, context: &mut C) {
        let buf = self.values.get(context);
        if let Some(len) = self.last_known_len {
            buf.truncate(len);
        }        

        buf.extend(self.removing.drain(..));        

        if !buf.is_empty() {
            self.last_known_len = Some(buf.len());
        } else {
            // The list should have been empty to start with.
            debug_assert!(self.last_known_len.is_none());
        }

        // Refresh the table state.
        if !buf.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn commit_changes(&mut self, context: &mut C) {
        self.last_known_len = Some(self.records(context).len());

        self.removing.clear();
    }

    pub fn push(&mut self, context: &mut C, new: T) {
        self.values.get(context).push(new);

        // In case, the list was emptied.
        if self.list_state.selected().is_none() {
            self.list_state.select(Some(0));
        }
    }

    pub fn add_and_commit(&mut self, context: &mut C, records: Vec<T>) {
        for r in records {
            self.push(context, r);
        }

        self.last_known_len = Some(self.records(context).len());
    }
    
    pub fn add_record(&mut self, context: &mut C, record: T) {
        self.push(context, record);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_list_new() {
        struct Context {
            value: Vec<i32>,
        }

        let list = ContextList::<Context, _>::new(|c| &mut c.value, "test");
        assert_eq!(list.name, "test");
    }
}