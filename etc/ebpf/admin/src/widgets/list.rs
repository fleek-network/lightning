use std::fmt::Display;

use color_eyre::Report;
use ebpf_service::{map, ConfigSource};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::{Modifier, Style};
use ratatui::widgets::{Block, Borders, ListItem, ListState};

use crate::action::Action;
use crate::components::profile::Profile;
use crate::config::Config;
use crate::tui::Frame;

#[derive(Default)]
pub struct List<T> {
    records: Vec<(bool, T)>,
    removing: Vec<(bool, T)>,
    name: &'static str,
    list_state: ListState,
}

impl<'a, T> List<T>
where
    T: Display,
{
    pub fn new(name: &'static str) -> Self {
        Self {
            records: Vec::new(),
            removing: Vec::new(),
            name,
            list_state: ListState::default().with_selected(None),
        }
    }

    pub fn get(&self) -> Option<&T> {
        self.records
            .get(self.list_state.selected()?)
            .map(|(_, r)| r)
    }

    pub fn update_state(&mut self, records: Vec<T>) {
        for r in records {
            self.push_record(false, r);
        }
    }

    pub fn scroll_up(&mut self) {
        if let Some(cur) = self.list_state.selected() {
            if cur > 0 {
                let cur = cur - 1;
                self.list_state.select(Some(cur));
            }
        }
    }

    pub fn scroll_down(&mut self) {
        if let Some(cur) = self.list_state.selected() {
            let len = self.records.len();
            if len > 0 && cur < len - 1 {
                let cur = cur + 1;
                self.list_state.select(Some(cur));
            }
        }
    }

    pub fn remove_cur(&mut self) {
        if let Some(cur) = self.list_state.selected() {
            debug_assert!(cur < self.records.len());
            let removing = self.records.remove(cur);
            self.removing.push(removing);

            if self.records.is_empty() {
                self.list_state.select(None);
            } else if cur == self.records.len() {
                self.list_state.select(Some(cur - 1));
            } else {
                self.list_state.select(Some(cur));
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
            self.list_state.select(Some(0));
        }
    }

    pub fn commit_changes(&mut self) {
        self.records.iter_mut().for_each(|(new, r)| {
            *new = false;
        });
        self.removing.clear();
    }

    pub fn add_record(&mut self, record: T) {
        self.push_record(true, record);
    }

    pub fn push_record(&mut self, new: bool, record: T) {
        self.records.push((new, record));

        // In case, the list was emptied.
        if self.list_state.selected().is_none() {
            debug_assert!(self.records.len() == 1);
            self.list_state.select(Some(0));
        }
    }

    pub fn state(&mut self) -> &mut ListState {
        &mut self.list_state
    }

    pub fn render(&mut self, f: &mut Frame<'_>, area: Rect) -> color_eyre::Result<()> {
        // Display profiles.
        let chunks = Layout::horizontal([Constraint::Percentage(100)]).split(area);
        let profiles = self
            .records
            .iter()
            .map(|(_, name)| format!("{}", name))
            .collect::<Vec<_>>();
        let profiles = ratatui::widgets::List::new(profiles)
            .block(Block::default().borders(Borders::ALL).title("Profiles"))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .highlight_symbol("> ");

        // Todo: maybe add a render method in custom widgets?
        f.render_stateful_widget(profiles, chunks[0], &mut self.list_state);

        Ok(())
    }
}
