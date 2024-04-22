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
    form: RuleForm,
    profile: Option<Profile>,
    src: ConfigSource,
    config: Config,
}

impl ProfileView {
    pub fn new(src: ConfigSource) -> Self {
        Self {
            command_tx: None,
            longest_item_per_column: [0; COLUMN_COUNT],
            table: Table::new(),
            form: RuleForm::new(),
            profile: None,
            src,
            config: Config::default(),
        }
    }

    pub fn load_profile(&mut self, profile: Profile) {
        self.table.load_records(profile.file_rules.clone());
        self.profile = Some(profile);
    }

    pub fn rule_form(&mut self) -> &mut RuleForm {
        &mut self.form
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

    fn update_rules_from_input(&mut self) {
        if let Some(rule) = self.form.yank_input() {
            // Update the profile in view.
            let profile = self.profile.as_mut().expect("Profile to be initialized");
            profile.file_rules.push(rule.clone());
            // Update the rule table.
            self.table.add_record(rule);
        }
    }

    fn restore(&mut self) {
        self.table.restore_state();
    }

    fn save(&mut self) {
        self.table.commit_changes();
        let records = self.table.records().cloned().collect::<Vec<_>>();
        let mut profile = self
            .profile
            .take()
            .expect("The view should laways have a profile to view");
        // Update profile with the new rules.
        profile.file_rules = records;
        let src = self.src.clone();
        tokio::spawn(async move {
            if let Err(e) = src.write_profiles(vec![profile]).await {
                error!("failed to write to list");
            }
        });
    }

    fn clear(&mut self) {
        self.profile.take();
        self.table.clear();
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
            Action::Add => Ok(Some(Action::UpdateMode(Mode::ProfileRuleForm))),
            Action::Remove => {
                self.table.remove_selected_record();
                Ok(Some(Action::Render))
            },
            Action::Save => {
                self.save();
                Ok(Some(Action::UpdateMode(Mode::ProfileView)))
            },
            Action::Cancel => {
                self.restore();
                Ok(Some(Action::UpdateMode(Mode::ProfileView)))
            },
            Action::UpdateMode(Mode::ProfileViewEdit) => {
                self.update_rules_from_input();
                Ok(None)
            },
            Action::Back => {
                self.clear();
                Ok(Some(Action::UpdateMode(Mode::Profiles)))
            },
            Action::Up => {
                self.table.scroll_up();
                Ok(Some(Action::Render))
            },
            Action::Down => {
                self.table.scroll_down();
                Ok(Some(Action::Render))
            },
            Action::NavLeft | Action::NavRight => {
                self.clear();
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
