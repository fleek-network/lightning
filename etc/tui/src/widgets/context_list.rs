use std::fmt::Display;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::{Modifier, Style};
use ratatui::widgets::{Block, Borders, ListState};

use crate::components::{Draw, DynExtractor, Extractor};
use crate::helpers::StableVec;
use crate::tui::Frame;

/// A list that tracks items edits of a list of items
/// and depends on a [StableVec] for the actual storage of the items
pub struct ContextList<C, T> {
    values: DynExtractor<C, StableVec<T>>,
    last_known_len: Option<usize>,
    removing: Vec<T>,
    name: &'static str,
    list_state: ListState,
}

impl<C, T> ContextList<C, T> {
    /// Create a new instance of a ContextList
    ///
    /// ```
    /// struct Context {
    ///     value: Vec<i32>,
    /// }
    ///
    /// let list = ContextList::<Context, _>::new("test_context", |c: &mut Context| &mut c.value);
    /// ```
    pub fn new<F>(name: &'static str, vals: F) -> Self
    where
        F: Extractor<C, StableVec<T>> + 'static,
    {
        Self {
            values: Box::new(vals),
            last_known_len: None,
            removing: Vec::new(),
            name,
            list_state: ListState::default().with_selected(None),
        }
    }

    pub fn selected(&self) -> Option<usize> {
        self.list_state.selected()
    }

    pub fn get_selected<'a>(&self, context: &'a mut C) -> Option<&'a T> {
        self.values.get(context).get(self.list_state.selected()?)
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

            let values = unsafe { self.values.get(context).as_vec() };

            let removing = values.remove(cur);
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

    pub fn removing(&self) -> &Vec<T> {
        &self.removing
    }

    pub fn uncommitted<'a>(&self, context: &'a mut C) -> &'a [T] {
        match self.last_known_len {
            Some(len) => {
                let lower = len.checked_sub(1).unwrap_or(0);
                &self.records(context)[lower..]
            },
            None => &[],
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
            self.list_state.select(Some(0));
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

    pub fn push(&mut self, context: &mut C, new: T) {
        self.values.get(context).push(new);

        // In case, the list was emptied.
        if self.list_state.selected().is_none() {
            self.list_state.select(Some(0));
        }
    }

    pub fn add_records_and_commit(&mut self, context: &mut C, records: Vec<T>) {
        for r in records {
            self.push(context, r);
        }

        self.last_known_len = Some(self.records(context).len());
    }

    pub fn add_record(&mut self, context: &mut C, record: T) {
        self.push(context, record);
    }
}

impl<C, T: Display> Draw for ContextList<C, T> {
    type Context = C;

    fn draw(&mut self, context: &mut Self::Context, f: &mut Frame<'_>, area: Rect) -> anyhow::Result<()> {
        let values = self.values.get(context);

        if self.selected().is_none() && !values.is_empty() {
            self.list_state.select(Some(0));
        }

        // Display profiles.
        let chunks = Layout::horizontal([Constraint::Percentage(100)]).split(area);
        let values = values
            .iter()
            .map(|name| format!("{}", name))
            .collect::<Vec<_>>();

        let values = ratatui::widgets::List::new(values)
            .block(Block::default().borders(Borders::ALL).title(self.name))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .highlight_symbol("> ");

        // Todo: maybe add a render method in custom widgets?
        f.render_stateful_widget(values, chunks[0], &mut self.list_state);

        Ok(())
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

    #[test]
    fn test_correctly_restores_state() {}

    #[test]
    fn test_correctly_commits_changes() {}
}
