use color_eyre::Report;
use ebpf_service::{map, ConfigSource};
use ratatui::widgets::TableState;

use crate::action::Action;
use crate::components::profile::Profile;
use crate::config::Config;

pub struct List<T> {
    records: Vec<(bool, T)>,
    removing: Vec<(bool, T)>,
    list_state: TableState,
}

impl<T> List<T> {
    pub fn new(src: ConfigSource) -> Self {
        Self {
            records: Vec::new(),
            removing: Vec::new(),
            list_state: TableState::default().with_selected(Some(0)),
        }
    }

    fn scroll_up(&mut self) {
        if let Some(cur) = self.list_state.selected() {
            if cur > 0 {
                let cur = cur - 1;
                self.list_state.select(Some(cur));
            }
        }
    }

    fn scroll_down(&mut self) {
        if let Some(cur) = self.list_state.selected() {
            let len = self.records.len();
            if len > 0 && cur < len - 1 {
                let cur = cur + 1;
                self.list_state.select(Some(cur));
            }
        }
    }

    fn remove_profile(&mut self) {
        if let Some(cur) = self.list_state.selected() {
            debug_assert!(cur < self.records.len());
            let removing = self.records.remove(cur);

            if self.records.is_empty() {
                self.list_state.select(None);
            } else if cur == self.records.len() {
                self.list_state.select(Some(cur - 1));
            } else {
                self.list_state.select(Some(cur));
            }

            self.removing.push(removing);
        }
    }

    pub fn restore_state(&mut self) {
        self.records.retain(|(new, _)| !new);
        self.removing.retain(|(new, _)| !new);
        self.records.append(&mut self.removing);
        self.removing.clear();

        // Refresh the table state.
        if !self.records.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    fn commit_changes(&mut self) {
        self.records.iter_mut().for_each(|(new, r)| {
            *new = false;
        });
        self.removing.clear();
    }

    fn new_rule(&mut self, profile: T) {
        self.records.push((true, profile));

        // In case, the list was emptied.
        if self.list_state.selected().is_none() {
            debug_assert!(self.records.len() == 1);
            self.list_state.select(Some(0));
        }
    }
}
