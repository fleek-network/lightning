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
use crate::components::{profile, Draw, DynExtractor};
use crate::config::{ComponentKeyBindings, Config};
use crate::helpers::StableVec;
use crate::widgets::context_table::Table;

fn space_between_columns(rules: &Vec<FileRule>) -> [u16; COLUMN_COUNT] {
    let name = rules
        .iter()
        .map(|r| r.file.display().to_string().as_str().width())
        .max()
        .unwrap_or(0);

    let permissions = rules
        .iter()
        .map(|r| r.permissions().as_str().width())
        .max()
        .unwrap_or(0);

    [name as u16, permissions as u16]
}

const COLUMN_COUNT: usize = 2;

pub struct ProfileView {
    profile_extractor: DynExtractor<ProfileContext, Profile>,
    rules_extractor: DynExtractor<ProfileContext, StableVec<FileRule>>,
    table: crate::widgets::context_table::Table<ProfileContext, FileRule>,
    longest_item_per_column: [u16; COLUMN_COUNT],
    src: ConfigSource,
    key_bindings: ComponentKeyBindings<ProfileViewActions>,
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
    pub fn new(src: ConfigSource) -> Self {
        Self {
            table: Table::new("ProfileView", |context: &mut ProfileContext| {
                &mut context
                    .view_profile
                    .as_mut()
                    .expect("A profile to be loaded")
                    .1
            }),
            rules_extractor: Box::new(|context: &mut ProfileContext| {
                &mut context
                    .view_profile
                    .as_mut()
                    .expect("A profile to be loaded")
                    .1
            }),
            profile_extractor: Box::new(|context: &mut ProfileContext| {
                let idx = context
                    .view_profile
                    .as_mut()
                    .expect("A profile to be loaded")
                    .0;

                context
                    .profiles
                    .get_mut(idx)
                    .expect("A loaded view idx to correspond to a profile")
            }),
            longest_item_per_column: [0; COLUMN_COUNT],
            src,
            key_bindings: Default::default(),
        }
    }

    fn save(&mut self, context: &mut ProfileContext) {
        let mut profile = self
            .profile_extractor
            .get(context)
            .clone();

        let rules = self
            .rules_extractor
            .get(context);

        profile.file_rules = rules.iter().cloned().collect();

        let src = self.src.clone();
        tokio::spawn(async move {
            if let Err(e) = src.write_profiles(vec![profile]).await {
                error!("failed to write to list: {e:?}");
            }
        });
    }
}

impl Component for ProfileView {
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
                    self.table.remove_selected_record(context);
                },
                ProfileViewActions::Save => {
                    self.save(context);

                    context.mounted = super::ProfileSubComponent::ProfileView;
                },
                ProfileViewActions::Cancel => {
                    self.table.restore_state(context);

                    context.mounted = super::ProfileSubComponent::ProfileView;
                },
                ProfileViewActions::Back => {
                    context.view_profile = None;

                    context.mounted = super::ProfileSubComponent::Profiles;
                },
                ProfileViewActions::Up => {
                    self.table.scroll_up();
                },
                ProfileViewActions::Down => {
                    self.table.scroll_down(context);
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
    type Context = ProfileContext;

    fn draw(&mut self, context: &mut Self::Context, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let rules = self.rules_extractor.get(context);

        if self.table.state().selected().is_none() && !rules.is_empty() {
            self.table.state().select(Some(0));
        }

        self.longest_item_per_column = space_between_columns(rules);
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

        let rows = rules.iter().map(|data| {
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
