use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;
use std::str::FromStr;

use anyhow::Result;
use lightning_guard::map::{FileRule, Profile};
use lightning_guard::ConfigSource;
use log::error;
use ratatui::prelude::{Color, Constraint, Modifier, Rect, Style, Text};
use ratatui::widgets::{Cell, Row};
use tokio::sync::mpsc::UnboundedSender;
use unicode_width::UnicodeWidthStr;

use super::forms::RuleForm;
use super::{Component, Frame, ProfileContext};
use crate::app::GlobalAction;
use crate::components::Draw;
use crate::config::{ComponentKeyBindings, Config};
use crate::widgets::table::Table;

pub struct ProfileViewContext {
    pub table: Rc<RefCell<Table<FileRule>>>,
    pub profile: Option<Profile>,
}

impl ProfileViewContext {
    pub(crate) fn update_rules_from_input(&mut self, rule: FileRule) {
        // Update the profile in view.
        let profile = self.profile.as_mut().expect("Profile to be initialized");
        profile.file_rules.push(rule.clone());

        // Update the rule table.
        self.table.deref().borrow_mut().add_record(rule);
    }

    pub fn load_profile(&mut self, profile: Profile) {
        self.table
            .deref()
            .borrow_mut()
            .load_records(profile.file_rules.clone());
        self.profile = Some(profile);
    }

    fn restore(&mut self) {
        self.table.deref().borrow_mut().restore_state();
    }

    fn save(&mut self) -> Profile {
        self.table.deref().borrow_mut().commit_changes();
        let records = self.table.borrow().records().cloned().collect::<Vec<_>>();
        let mut profile = self
            .profile
            .clone()
            .expect("The view should laways have a profile to view");

        // Update profile with the new rules.
        profile.file_rules = records;

        profile
    }

    fn clear(&mut self) {
        self.profile.take();
        self.table.deref().borrow_mut().clear();
    }
}

const COLUMN_COUNT: usize = 2;

pub struct ProfileView {
    table: Rc<RefCell<Table<FileRule>>>,
    src: ConfigSource,
    key_bindings: ComponentKeyBindings<ProfileViewActions>,
    longest_item_per_column: [u16; COLUMN_COUNT],
}

pub enum ProfileViewActions {
    Edit,
    Add,
    Remove,
    Save,
    Cancel,
    Back,
    Up,
    Down,
    Suspend,
    Quit,
}

impl FromStr for ProfileViewActions {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "Edit" => Ok(Self::Edit),
            "Add" => Ok(Self::Add),
            "Remove" => Ok(Self::Remove),
            "Save" => Ok(Self::Save),
            "Cancel" => Ok(Self::Cancel),
            "Back" => Ok(Self::Back),
            "Up" => Ok(Self::Up),
            "Down" => Ok(Self::Down),
            "Suspend" => Ok(Self::Suspend),
            "Quit" => Ok(Self::Quit),
            _ => Err(anyhow::anyhow!("Invalid ProfileViewActions {s}")),
        }
    }
}

impl ProfileView {
    fn space_between_columns(&self) -> [u16; COLUMN_COUNT] {
        let name = self
            .table
            .borrow()
            .records()
            .map(|r| r.file.display().to_string().as_str().width())
            .max()
            .unwrap_or(0);

        let permissions = self
            .table
            .borrow()
            .records()
            .map(|r| r.permissions().as_str().width())
            .max()
            .unwrap_or(0);

        [name as u16, permissions as u16]
    }

    pub fn new(src: ConfigSource, context: &ProfileViewContext) -> Self {
        Self {
            src,
            key_bindings: Default::default(),
            table: context.table.clone(),
            longest_item_per_column: [0, 0],
        }
    }

    fn save(&mut self, context: &mut ProfileViewContext) {
        let profile = context.save();

        let src = self.src.clone();
        tokio::spawn(async move {
            if let Err(e) = src.write_profiles(vec![profile]).await {
                error!("failed to write to list: {e:?}");
            }
        });
    }
}

impl Component for ProfileView {
    type Context = ProfileContext;

    fn component_name(&self) -> &'static str {
        "ProfileView"
    }

    fn register_keybindings(&mut self, config: &Config) {
        self.key_bindings
            .extend(config.keybindings.parse_actions("ProfileViewEdit"));
        self.key_bindings
            .extend(config.keybindings.parse_actions(self.component_name()));
    }

    fn handle_event(
        &mut self,
        context: &mut Self::Context,
        event: &[crossterm::event::KeyEvent],
    ) -> Result<Option<crate::app::GlobalAction>> {
        if let Some(action) = self.key_bindings.get(event) {
            match action {
                ProfileViewActions::Edit => {
                    context.mounted = super::ProfileSubComponent::ProfileViewEdit;
                },
                ProfileViewActions::Add => {
                    context.mounted = super::ProfileSubComponent::ProfileRuleForm;
                },
                ProfileViewActions::Remove => {
                    self.table.deref().borrow_mut().remove_selected_record();
                },
                ProfileViewActions::Save => {
                    self.save(&mut context.profile_view_context);

                    context.mounted = super::ProfileSubComponent::ProfileView;
                },
                ProfileViewActions::Cancel => {
                    context.profile_view_context.restore();

                    context.mounted = super::ProfileSubComponent::ProfileView;
                },
                ProfileViewActions::Back => {
                    context.profile_view_context.clear();

                    context.mounted = super::ProfileSubComponent::Profiles;
                },
                ProfileViewActions::Up => {
                    self.table.deref().borrow_mut().scroll_up();
                },
                ProfileViewActions::Down => {
                    self.table.deref().borrow_mut().scroll_down();
                },
                _ => return Ok(None),
            }

            return Ok(Some(GlobalAction::Render));
        }

        Ok(None)
    }
}

fn flatten_filter(filter: &FileRule) -> [String; COLUMN_COUNT] {
    [filter.file.display().to_string(), filter.permissions()]
}

impl Draw for ProfileView {
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let column_names = ["Target", "Permissions"];
        debug_assert!(column_names.len() == COLUMN_COUNT);

        self.longest_item_per_column = self.space_between_columns();
        debug_assert!(self.longest_item_per_column.len() == COLUMN_COUNT);

        let header_style = Style::default().fg(Color::White).bg(Color::Blue);
        let selected_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(Color::DarkGray);
        let header = column_names
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style);

        let table = self.table.deref().borrow();
        let rows = table
            .records()
            .map(|data| {
                let item = flatten_filter(data);
                item.into_iter()
                    .map(|content| {
                        let text = Text::from(content);
                        Cell::from(text)
                    })
                    .collect::<Row>()
                    .style(Style::new().fg(Color::White).bg(Color::Black))
            })
            .collect::<Vec<_>>();
        drop(table);

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
        
        let mut ref_table = self.table.deref().borrow_mut();
        f.render_stateful_widget(table, area, ref_table.state());

        Ok(())
    }
}
