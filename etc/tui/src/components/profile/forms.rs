use std::str::FromStr;

use anyhow::Result;
use crossterm::event::KeyEvent;
use lightning_guard::map;
use lightning_guard::map::FileRule;
use ratatui::prelude::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::Clear;
use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::{Input, TextArea};

use super::{Component, Frame, ProfileContext};
use crate::app::GlobalAction;
use crate::components::{Draw, DynExtractor, Extractor};
use crate::config::{ComponentKeyBindings, Config};
use crate::helpers::StableVec;
use crate::widgets::utils;
use crate::widgets::utils::InputField;

const PROFILE_NAME: &str = "Profile Name";
const TARGET_NAME: &str = "Target";
const PERMISSIONS: &str = "Permissions";
const INPUT_FORM_X: u16 = 20;
const INPUT_FORM_Y: u16 = 40;
const PROFILE_NAME_INPUT_FIELD_COUNT: usize = 1;
const RULE_INPUT_FIELD_COUNT: usize = 2;

pub enum ProfileFormActions {
    Cancel,
    Add,
    Up,
    Down,
    Suspend,
    Quit,
}

impl FromStr for ProfileFormActions {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "Cancel" => Ok(Self::Cancel),
            "Add" => Ok(Self::Add),
            "Up" => Ok(Self::Up),
            "Down" => Ok(Self::Down),
            "Suspend" => Ok(Self::Suspend),
            "Quit" => Ok(Self::Quit),
            _ => Err(anyhow::anyhow!("Unknown action: {}", s)),
        }
    }
}

#[derive(Default)]
pub struct ProfileForm {
    input_fields: Vec<InputField>,
    selected_input_field: usize,
    buf: Option<map::Profile>,
    keybindings: ComponentKeyBindings<ProfileFormActions>,
}

impl ProfileForm {
    pub fn new() -> Self {
        let input_fields: Vec<_> = vec![(PROFILE_NAME, TextArea::default())]
            .into_iter()
            .map(|(title, area)| InputField { title, area })
            .collect();

        Self {
            input_fields,
            selected_input_field: 0,
            buf: None,
            keybindings: Default::default(),
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
        let profile = map::Profile {
            name: Some(name.as_str().into()),
            ..Default::default()
        };
        self.buf.replace(profile);

        Ok(())
    }

    pub fn yank_input(&mut self) -> Option<map::Profile> {
        self.buf.take()
    }
}

impl Component for ProfileForm {
    fn component_name(&self) -> &'static str {
        "ProfileForm"
    }

    fn register_keybindings(&mut self, config: &Config) {
        self.keybindings = config.keybindings.parse_actions(self.component_name());
    }

    fn handle_event(
        &mut self,
        context: &mut Self::Context,
        event: &[KeyEvent],
    ) -> Result<Option<crate::app::GlobalAction>> {
        if event.len() == 1 {
            self.selected_field().area.input(event[0]);
        }

        if let Some(action) = self.keybindings.get(event) {
            match action {
                ProfileFormActions::Add => {
                    context.mounted = super::ProfileSubComponent::ProfilesEdit;

                    if let Err(e) = self.update_profile_from_input() {
                        return Err(anyhow::anyhow!("creating profile failed {e}"));
                    } else {
                        if let Some(profile) = self.yank_input() {
                            context.profiles.push(profile);
                        } else {
                            log::error!("Failed to create profile. This is a bug");
                        }
                    }
                },
                ProfileFormActions::Cancel => {
                    context.mounted = super::ProfileSubComponent::ProfilesEdit;
                    self.clear_input();
                },
                ProfileFormActions::Up => {
                    if self.selected_input_field > 0 {
                        self.selected_input_field -= 1;
                    }
                },
                ProfileFormActions::Down => {
                    if self.selected_input_field < self.input_fields.len() - 1 {
                        self.selected_input_field += 1;
                    }
                },
                _ => {},
            }
        }

        Ok(Some(crate::app::GlobalAction::Render))
    }
}

impl Draw for ProfileForm {
    type Context = ProfileContext;

    fn draw(&mut self, _context: &mut Self::Context, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        // Display form to enter new rule.
        debug_assert!(self.input_fields.len() == PROFILE_NAME_INPUT_FIELD_COUNT);

        f.render_widget(Clear, area);
        let area = utils::center_form(INPUT_FORM_X, INPUT_FORM_Y, area);

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
                utils::activate(textarea);
            } else {
                utils::inactivate(textarea)
            }
            let widget = textarea.area.widget();
            f.render_widget(widget, *chunk);
        }

        Ok(())
    }
}

pub struct RuleForm {
    input_fields: Vec<InputField>,
    rules_extractor: DynExtractor<ProfileContext, StableVec<FileRule>>,
    selected_input_field: usize,
    buf: Option<FileRule>,
    keybindings: ComponentKeyBindings<RuleFormActions>,
}

#[derive(Debug)]
pub enum RuleFormActions {
    Cancel,
    Add,
    Up,
    Down,
    Suspend,
    Quit,
}

impl FromStr for RuleFormActions {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "Cancel" => Ok(Self::Cancel),
            "Add" => Ok(Self::Add),
            "Up" => Ok(Self::Up),
            "Down" => Ok(Self::Down),
            "Suspend" => Ok(Self::Suspend),
            "Quit" => Ok(Self::Quit),
            _ => Err(anyhow::anyhow!("Unknown Rule Form Action: {}", s)),
        }
    }
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
        utils::activate(&mut input_fields[0]);
        utils::inactivate(&mut input_fields[1]);

        Self {
            keybindings: Default::default(),
            rules_extractor: Box::new(|c: &mut ProfileContext| &mut c.view_profile.as_mut().expect("A profile to be loaded").1),
            input_fields,
            selected_input_field: 0,
            buf: None,
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

        let perm_input = self.input_fields[1].area.yank_text().trim().to_lowercase();
        let mut permissions = FileRule::NO_OPERATION;
        for op in perm_input.chars() {
            if op == FileRule::OPEN_SYMBOL {
                permissions |= FileRule::OPEN_MASK;
            }
            if op == FileRule::READ_SYMBOL {
                permissions |= FileRule::READ_MASK;
            }
            if op == FileRule::WRITE_SYMBOL {
                permissions |= FileRule::WRITE_MASK;
            }
        }

        self.buf.replace(FileRule {
            file: self.input_fields[0].area.yank_text().trim().try_into()?,
            permissions,
        });

        Ok(())
    }
}

impl Draw for RuleForm {
    type Context = ProfileContext;

    fn draw(&mut self, _context: &mut Self::Context, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        // Display form to enter new rule.
        debug_assert!(self.input_fields.len() == RULE_INPUT_FIELD_COUNT);

        f.render_widget(Clear, area);
        let area = utils::center_form(INPUT_FORM_X, INPUT_FORM_Y, area);

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
                utils::activate(textarea);
            } else {
                utils::inactivate(textarea)
            }
            let widget = textarea.area.widget();
            f.render_widget(widget, *chunk);
        }

        Ok(())
    }
}

impl Component for RuleForm {
    fn component_name(&self) -> &'static str {
        "ProfileRuleForm"
    }

    fn register_keybindings(&mut self, config: &Config) {
        self.keybindings = config.keybindings.parse_actions(self.component_name());
    }

    fn handle_event(
        &mut self,
        context: &mut Self::Context,
        event: &[KeyEvent],
    ) -> Result<Option<crate::app::GlobalAction>> {
        if event.len() == 1 {
            self.selected_field().area.input(event[0]);
        }

        if let Some(action) = self.keybindings.get(event) {
            match action {
                RuleFormActions::Add => {
                    if let Err(e) = self.update_filters_from_input() {
                        return Err(anyhow::anyhow!("creating rule failed {e}"));
                    }

                    context.mounted = super::ProfileSubComponent::ProfileViewEdit;
                    self.rules_extractor.get(context).push(
                        self.buf
                            .take()
                            .ok_or(anyhow::anyhow!("No rule found to add to buf"))?,
                    );
                },
                RuleFormActions::Cancel => {
                    context.mounted = super::ProfileSubComponent::ProfileViewEdit;
                    self.clear_input();
                },
                RuleFormActions::Up => {
                    if self.selected_input_field > 0 {
                        self.selected_input_field -= 1;
                    }
                },
                RuleFormActions::Down => {
                    if self.selected_input_field < self.input_fields.len() - 1 {
                        self.selected_input_field += 1;
                    }
                },
                RuleFormActions::Quit => return Ok(Some(GlobalAction::Quit)),
                _ => {},
            }
        }

        Ok(Some(crate::app::GlobalAction::Render))
    }
}
