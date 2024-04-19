use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

use color_eyre::eyre::Result;
use color_eyre::owo_colors::OwoColorize;
use color_eyre::Report;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ebpf_service::map::FileRule;
use ebpf_service::ConfigSource;
use log::error;
use ratatui::prelude::*;
use ratatui::widgets::*;
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;
use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::{Input, TextArea};
use unicode_width::UnicodeWidthStr;

use super::{Component, Frame};
use crate::action::Action;
use crate::config::{Config, KeyBindings};
use crate::mode::Mode;

const COLUMN_COUNT: usize = 2;

#[derive(Default)]
pub struct ProfileView {
    command_tx: Option<UnboundedSender<Action>>,
    filters: Vec<(bool, FileRule)>,
    removing: Vec<(bool, FileRule)>,
    longest_item_per_column: [u16; COLUMN_COUNT],
    table_state: TableState,
    config: Config,
}

impl ProfileView {
    pub fn new() -> Self {
        let filters = vec![
            (
                false,
                FileRule {
                    file: "~/.lightning/keystore/consensus.pem".try_into().unwrap(),
                    operations: FileRule::READ_MASK | FileRule::WRITE_MASK,
                },
            ),
            (
                false,
                FileRule {
                    file: "~/.lightning/keystore/node.pem".try_into().unwrap(),
                    operations: FileRule::READ_MASK,
                },
            ),
            (
                false,
                FileRule {
                    file: "~/.lightning/keystore/node.pem".try_into().unwrap(),
                    operations: FileRule::NO_OPERATION,
                },
            ),
        ];
        Self {
            filters,
            removing: Vec::new(),
            command_tx: None,
            longest_item_per_column: [0; COLUMN_COUNT],
            table_state: TableState::default().with_selected(0),
            config: Config::default(),
        }
    }

    fn scroll_up(&mut self) {
        if let Some(cur) = self.table_state.selected() {
            if cur > 0 {
                let cur = cur - 1;
                self.table_state.select(Some(cur));
            }
        }
    }

    fn scroll_down(&mut self) {
        if let Some(cur) = self.table_state.selected() {
            let len = self.filters.len();
            if len > 0 && cur < len - 1 {
                let cur = cur + 1;
                self.table_state.select(Some(cur));
            }
        }
    }

    fn remove_rule(&mut self) {
        let mut elem = None;
        if let Some(cur) = self.table_state.selected() {
            debug_assert!(cur < self.filters.len());
            elem = Some(self.filters.remove(cur));

            if self.filters.is_empty() {
                self.table_state.select(None);
            } else if cur == self.filters.len() {
                self.table_state.select(Some(cur - 1));
            } else {
                self.table_state.select(Some(cur));
            }
        }
        if let Some((new, rule)) = elem {
            self.removing.push((new, rule));
        }
    }

    pub fn restore_state(&mut self) {
        self.filters.retain(|(new, _)| !new);
        self.removing.retain(|(new, _)| !new);
        // Todo: avoid copying here.
        self.filters.extend(self.removing.clone().into_iter());
        self.removing.clear();

        // Refresh the table state.
        if !self.filters.is_empty() {
            self.table_state.select(Some(0));
        }
    }

    fn commit_changes(&mut self) {
        self.filters.iter_mut().for_each(|(new, r)| {
            *new = false;
        });
        self.removing.clear();
    }

    fn new_rule(&mut self, rule: FileRule) {
        self.filters.push((true, rule));

        // In case, the list was emptied.
        if self.table_state.selected().is_none() {
            debug_assert!(self.filters.len() == 1);
            self.table_state.select(Some(0));
        }
    }
}

impl Component for ProfileView {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Edit => Ok(None),
            Action::Add => Ok(None),
            Action::Save => {
                self.commit_changes();
                // self.update_storage();
                Ok(Some(Action::UpdateMode(Mode::Firewall)))
            },
            Action::Cancel => {
                self.restore_state();
                Ok(Some(Action::UpdateMode(Mode::Firewall)))
            },
            Action::Remove => {
                self.remove_rule();
                Ok(Some(Action::Render))
            },
            Action::Up => {
                self.scroll_up();
                Ok(Some(Action::Render))
            },
            Action::Down => {
                self.scroll_down();
                Ok(Some(Action::Render))
            },
            _ => Ok(None),
        }
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        self.longest_item_per_column = space_between_columns(&self.filters);
        debug_assert!(self.longest_item_per_column.len() == COLUMN_COUNT);

        let column_names = ["Target", "Permissions"];
        debug_assert!(column_names.len() == COLUMN_COUNT);

        let header_style = Style::default().fg(Color::White).bg(Color::Blue);
        let selected_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(Color::DarkGray);
        let header = column_names
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style);

        let rows = self.filters.iter().enumerate().map(|(i, (_, data))| {
            let item = flatten_filter(data);
            item.into_iter()
                .map(|content| {
                    let text = Text::from(content);
                    Cell::from(text)
                })
                .collect::<Row>()
                .style(Style::new().fg(Color::White).bg(Color::Black))
        });

        let contraints = [
            Constraint::Min(self.longest_item_per_column[0] + 1),
            Constraint::Min(self.longest_item_per_column[1]),
        ];
        debug_assert!(contraints.len() == COLUMN_COUNT);

        let bar = " > ";
        let table = Table::new(rows, contraints)
            .header(header)
            .highlight_style(selected_style)
            .highlight_symbol(Text::from(bar));

        f.render_stateful_widget(table, area, &mut self.table_state);

        Ok(())
    }
}

fn space_between_columns(items: &Vec<(bool, FileRule)>) -> [u16; COLUMN_COUNT] {
    let name = items
        .iter()
        .map(|(_, r)| r.file.display().to_string().as_str().width())
        .max()
        .unwrap_or(0);
    let permissions = items
        .iter()
        .map(|(_, r)| r.permissions().as_str().width())
        .max()
        .unwrap_or(0);

    [name as u16, permissions as u16]
}

fn flatten_filter(filter: &FileRule) -> [String; COLUMN_COUNT] {
    [filter.file.display().to_string(), filter.permissions()]
}
