use std::fmt::Display;

use color_eyre::Report;
use ebpf_service::{map, ConfigSource};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::{Color, Modifier, Style, Text};
use ratatui::widgets::{Block, Borders, Cell, Row, TableState};

use crate::action::Action;
use crate::components::profile::Profile;
use crate::config::Config;
use crate::tui::Frame;

#[derive(Default)]
pub struct Table<T> {
    records: Vec<(bool, T)>,
    removing: Vec<(bool, T)>,
    state: TableState,
}

impl<T> Table<T> {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            removing: Vec::new(),
            state: TableState::default().with_selected(Some(0)),
        }
    }

    pub fn with_records(records: Vec<T>) -> Self {
        Self {
            records: records.into_iter().map(|r| (false, r)).collect(),
            removing: Default::default(),
            state: TableState::default().with_selected(Some(0)),
        }
    }

    pub fn update_state(&mut self, records: Vec<T>) {
        self.records = records.into_iter().map(|r| (false, r)).collect();
    }

    pub fn scroll_up(&mut self) {
        if let Some(cur) = self.state.selected() {
            if cur > 0 {
                let cur = cur - 1;
                self.state.select(Some(cur));
            }
        }
    }

    pub fn scroll_down(&mut self) {
        if let Some(cur) = self.state.selected() {
            let len = self.records.len();
            if len > 0 && cur < len - 1 {
                let cur = cur + 1;
                self.state.select(Some(cur));
            }
        }
    }

    pub fn remove_cur(&mut self) {
        if let Some(cur) = self.state.selected() {
            debug_assert!(cur < self.records.len());
            let removing = self.records.remove(cur);
            self.removing.push(removing);

            if self.records.is_empty() {
                self.state.select(None);
            } else if cur == self.records.len() {
                self.state.select(Some(cur - 1));
            } else {
                self.state.select(Some(cur));
            }
        }
    }

    pub fn restore_state(&mut self) {
        self.records.retain(|(new, _)| !new);
        self.removing.retain(|(new, _)| !new);
        self.records.append(&mut self.removing);
        self.removing.clear();

        // Refresh the table state.
        if !self.records.is_empty() {
            self.state.select(Some(0));
        }
    }

    pub fn commit_changes(&mut self) {
        self.records.iter_mut().for_each(|(new, r)| {
            *new = false;
        });
        self.removing.clear();
    }

    pub fn add_record(&mut self, record: T) {
        self.records.push((true, record));

        // In case, the list was emptied.
        if self.state.selected().is_none() {
            debug_assert!(self.records.len() == 1);
            self.state.select(Some(0));
        }
    }

    pub fn state(&mut self) -> &mut TableState {
        &mut self.state
    }

    pub fn records(&self) -> impl Iterator<Item = &T> {
        self.records.iter().map(|(_, r)| r)
    }
}
