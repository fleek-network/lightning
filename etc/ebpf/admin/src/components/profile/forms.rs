use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

use color_eyre::eyre::Result;
use color_eyre::owo_colors::OwoColorize;
use color_eyre::Report;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ebpf_service::map::{FileRule, PacketFilterRule};
use ebpf_service::{map, ConfigSource};
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

const PROFILE_NAME: &str = "Profile Name";
const TARGET_NAME: &str = "Target";
const PERMISSIONS: &str = "Permissions";
const INPUT_FORM_X: u16 = 20;
const INPUT_FORM_Y: u16 = 40;
const PROFILE_NAME_INPUT_FIELD_COUNT: usize = 1;
const RULE_INPUT_FIELD_COUNT: usize = 2;

#[derive(Default)]
pub struct ProfileForm {
    command_tx: Option<UnboundedSender<Action>>,
    input_fields: Vec<InputField>,
    selected_input_field: usize,
    buf: Option<map::Profile>,
    config: Config,
}

impl ProfileForm {
    pub fn new() -> Self {
        let input_fields: Vec<_> = vec![(PROFILE_NAME, TextArea::default())]
            .into_iter()
            .map(|(title, area)| InputField { title, area })
            .collect();

        Self {
            command_tx: None,
            input_fields,
            selected_input_field: 0,
            buf: None,
            config: Config::default(),
        }
    }

    fn selected_field(&mut self) -> &mut InputField {
        &mut self.input_fields[self.selected_input_field]
    }

    fn clear_input(&mut self) {
        for field in self.input_fields.iter_mut() {
            field.area.select_all();
            field.area.cut();
            field.area.yank_text();
        }
    }

    fn update_profile_from_input(&mut self) -> Result<()> {
        for field in self.input_fields.iter_mut() {
            field.area.select_all();
            field.area.cut();
        }

        let name = self.input_fields[0].area.yank_text().trim().to_string();
        let mut profile = map::Profile::default();
        profile.name = Some(name.as_str().try_into()?);
        self.buf.replace(profile);

        Ok(())
    }

    pub fn yank_input(&mut self) -> Option<map::Profile> {
        self.buf.take()
    }
}

impl Component for ProfileForm {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        self.selected_field().area.input(Input::from(key));
        Ok(None)
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Cancel => {
                self.clear_input();
                Ok(Some(Action::UpdateMode(Mode::ProfilesEdit)))
            },
            Action::Add => {
                if let Err(e) = self.update_profile_from_input() {
                    Ok(Some(Action::Error(e.to_string())))
                } else {
                    // Todo: Here send new entry.
                    Ok(Some(Action::UpdateMode(Mode::ProfilesEdit)))
                }
            },
            Action::Up => {
                if self.selected_input_field > 0 {
                    self.selected_input_field -= 1;
                }
                Ok(Some(Action::Render))
            },
            Action::Down => {
                if self.selected_input_field < self.input_fields.len() - 1 {
                    self.selected_input_field += 1;
                }
                Ok(Some(Action::Render))
            },
            _ => Ok(None),
        }
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        // Display form to enter new rule.
        debug_assert!(self.input_fields.len() == PROFILE_NAME_INPUT_FIELD_COUNT);

        f.render_widget(Clear, area);
        let area = center_form(INPUT_FORM_X, INPUT_FORM_Y, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Percentage(0),
                    Constraint::Length(3),
                    Constraint::Percentage(0),
                ]
                .as_ref(),
            )
            .split(area);

        for (i, (textarea, chunk)) in self
            .input_fields
            .iter_mut()
            // We don't want the first or last because they're for padding.
            .zip(chunks.iter().take(3).skip(1))
            .enumerate()
        {
            if i == self.selected_input_field {
                activate(textarea);
            } else {
                inactivate(textarea)
            }
            let widget = textarea.area.widget();
            f.render_widget(widget, *chunk);
        }

        Ok(())
    }
}

pub struct RuleForm {
    command_tx: Option<UnboundedSender<Action>>,
    input_fields: Vec<InputField>,
    selected_input_field: usize,
    buf: Option<FileRule>,
    config: Config,
}

impl RuleForm {
    pub fn new() -> Self {
        let mut input_fields: Vec<_> = vec![
            (TARGET_NAME, TextArea::default()),
            (PERMISSIONS, TextArea::default()),
        ]
        .into_iter()
        .map(|(title, area)| InputField { title, area })
        .collect();

        debug_assert!(input_fields.len() == RULE_INPUT_FIELD_COUNT);
        activate(&mut input_fields[0]);
        inactivate(&mut input_fields[1]);

        Self {
            command_tx: None,
            input_fields,
            selected_input_field: 0,
            buf: None,
            config: Config::default(),
        }
    }

    fn selected_field(&mut self) -> &mut InputField {
        &mut self.input_fields[self.selected_input_field]
    }

    fn clear_input(&mut self) {
        for field in self.input_fields.iter_mut() {
            field.area.select_all();
            field.area.cut();
            field.area.yank_text();
        }
    }

    fn update_filters_from_input(&mut self) -> Result<()> {
        for field in self.input_fields.iter_mut() {
            field.area.select_all();
            field.area.cut();
        }

        // Todo: get operations from permissions.
        let _permissions = self.input_fields[1].area.yank_text();
        self.buf.replace(FileRule {
            file: self.input_fields[0].area.yank_text().trim().try_into()?,
            operations: FileRule::NO_OPERATION,
        });

        Ok(())
    }

    pub fn yank_input(&mut self) -> Option<FileRule> {
        self.buf.take()
    }
}

impl Component for RuleForm {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        self.selected_field().area.input(Input::from(key));
        Ok(None)
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Cancel => {
                self.clear_input();
                Ok(Some(Action::UpdateMode(Mode::ProfileViewEdit)))
            },
            Action::Add => {
                if let Err(e) = self.update_filters_from_input() {
                    Ok(Some(Action::Error(e.to_string())))
                } else {
                    Ok(Some(Action::UpdateMode(Mode::ProfileViewEdit)))
                }
            },
            Action::Up => {
                if self.selected_input_field > 0 {
                    self.selected_input_field -= 1;
                }
                Ok(Some(Action::Render))
            },
            Action::Down => {
                if self.selected_input_field < self.input_fields.len() - 1 {
                    self.selected_input_field += 1;
                }
                Ok(Some(Action::Render))
            },
            _ => Ok(None),
        }
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        // Display form to enter new rule.
        debug_assert!(self.input_fields.len() == RULE_INPUT_FIELD_COUNT);

        f.render_widget(Clear, area);
        let area = center_form(INPUT_FORM_X, INPUT_FORM_Y, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Percentage(0),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Percentage(0),
                ]
                .as_ref(),
            )
            .split(area);

        for (i, (textarea, chunk)) in self
            .input_fields
            .iter_mut()
            // We don't want the first or last because they're for padding.
            .zip(chunks.iter().take(3).skip(1))
            .enumerate()
        {
            if i == self.selected_input_field {
                activate(textarea);
            } else {
                inactivate(textarea)
            }
            let widget = textarea.area.widget();
            f.render_widget(widget, *chunk);
        }

        Ok(())
    }
}

struct InputField {
    title: &'static str,
    area: TextArea<'static>,
}

fn inactivate(field: &mut InputField) {
    field.area.set_cursor_line_style(Style::default());
    field.area.set_cursor_style(Style::default());
    field.area.set_block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .title(field.title),
    );
}

fn activate(field: &mut InputField) {
    field
        .area
        .set_cursor_line_style(Style::default().add_modifier(Modifier::UNDERLINED));
    field
        .area
        .set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
    field.area.set_block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Red))
            .title(field.title),
    );
}

fn center_form(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
