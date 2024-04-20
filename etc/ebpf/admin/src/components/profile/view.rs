use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

use color_eyre::eyre::Result;
use color_eyre::owo_colors::OwoColorize;
use color_eyre::Report;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ebpf_service::map::{FileRule, Profile};
use ebpf_service::{map, ConfigSource};
use log::error;
use ratatui::prelude::{Color, Constraint, Modifier, Rect, Style, Text};
use ratatui::widgets::{Cell, Row};
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;
use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::{Input, TextArea};
use unicode_width::UnicodeWidthStr;

use super::{Component, Frame};
use crate::action::Action;
use crate::components::profile::forms::{ProfileForm, RuleForm};
use crate::config::{Config, KeyBindings};
use crate::mode::Mode;
use crate::widgets::table::Table;

const COLUMN_COUNT: usize = 2;

pub struct ProfileView {
    command_tx: Option<UnboundedSender<Action>>,
    longest_item_per_column: [u16; COLUMN_COUNT],
    table: Table<FileRule>,
    profile_form: ProfileForm,
    rule_form: RuleForm,
    profile: Option<Profile>,
    config: Config,
}

impl Default for ProfileView {
    fn default() -> Self {
        Self {
            command_tx: None,
            longest_item_per_column: [0; COLUMN_COUNT],
            table: Table::new(),
            profile_form: ProfileForm::new(),
            rule_form: RuleForm::new(),
            profile: None,
            config: Config::default(),
        }
    }
}

impl ProfileView {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_state(&mut self, profile: Profile) {
        self.table.update_state(profile.file_rules.clone());
        self.profile = Some(profile);
    }

    pub fn profile_form(&mut self) -> &mut ProfileForm {
        &mut self.profile_form
    }

    pub fn rule_form(&mut self) -> &mut RuleForm {
        &mut self.rule_form
    }

    fn space_between_columns(&self) -> [u16; COLUMN_COUNT] {
        let name = self
            .table
            .records()
            .map(|r| r.file.display().to_string().as_str().width())
            .max()
            .unwrap_or(0);
        let permissions = self
            .table
            .records()
            .map(|r| r.permissions().as_str().width())
            .max()
            .unwrap_or(0);

        [name as u16, permissions as u16]
    }

    fn update_profile_from_input(&mut self) {
        debug_assert!(self.profile.is_some());

        if let Some(name) = self.profile_form.yank_input() {
            let profile = self.profile.as_mut().expect("Profile to be initialized");
            profile.name = name.name
        }

        if let Some(rule) = self.rule_form.yank_input() {
            let profile = self.profile.as_mut().expect("Profile to be initialized");
            profile.file_rules.push(rule.clone());
            self.table.add_record(rule);
        }
    }

    pub fn yank_profile(&mut self) -> Option<Profile> {
        self.profile.take()
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
            Action::Edit => Ok(Some(Action::UpdateMode(Mode::ProfileViewEdit))),
            Action::Save => {
                self.table.commit_changes();
                // self.update_storage();
                Ok(Some(Action::UpdateMode(Mode::ProfilesEdit)))
            },
            Action::Cancel => {
                self.table.restore_state();
                Ok(Some(Action::UpdateMode(Mode::Profiles)))
            },
            Action::AddRule => Ok(Some(Action::UpdateMode(Mode::ProfileViewEditRuleForm))),
            Action::Remove => {
                self.table.remove_cur();
                Ok(Some(Action::Render))
            },
            Action::Up => {
                self.table.scroll_up();
                Ok(Some(Action::Render))
            },
            Action::Down => {
                self.table.scroll_down();
                Ok(Some(Action::Render))
            },
            Action::UpdateMode(Mode::ProfileViewEdit) => {
                self.update_profile_from_input();
                Ok(None)
            },
            Action::NavLeft | Action::NavRight => {
                self.profile.take();
                self.table.clear();
                Ok(None)
            },
            _ => Ok(None),
        }
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        self.longest_item_per_column = self.space_between_columns();
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

        let rows = self.table.records().enumerate().map(|(i, data)| {
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
        let table = ratatui::widgets::Table::new(rows, contraints)
            .header(header)
            .highlight_style(selected_style)
            .highlight_symbol(Text::from(bar));

        f.render_stateful_widget(table, area, self.table.state());

        Ok(())
    }
}

fn flatten_filter(filter: &FileRule) -> [String; COLUMN_COUNT] {
    [filter.file.display().to_string(), filter.permissions()]
}
