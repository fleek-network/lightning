use anyhow::Result;
use lightning_guard::map::{FileRule, Profile};
use lightning_guard::ConfigSource;
use log::error;
use ratatui::prelude::{Color, Constraint, Modifier, Rect, Style, Text};
use ratatui::widgets::{Cell, Row};
use tokio::sync::mpsc::UnboundedSender;
use unicode_width::UnicodeWidthStr;

use super::{Component, Frame, ProfileContext};
use crate::app::GlobalAction;
use crate::components::Draw;
use crate::config::{ComponentKeyBindings, Config};
use crate::widgets::table::Table;

const COLUMN_COUNT: usize = 2;

pub struct ProfileView {
    longest_item_per_column: [u16; COLUMN_COUNT],
    table: Table<FileRule>,
    profile: Option<Profile>,
    src: ConfigSource,
    config: Config,
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
    NavLeft,
    NavRight,
}

impl ProfileView {
    pub fn new(src: ConfigSource) -> Self {
        Self {
            longest_item_per_column: [0; COLUMN_COUNT],
            table: Table::new(),
            profile: None,
            src,
            config: Config::default(),
            key_bindings: Default::default(),
        }
    }

    pub fn load_profile(&mut self, profile: Profile) {
        self.table.load_records(profile.file_rules.clone());
        self.profile = Some(profile);
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
            .clone()
            .expect("The view should laways have a profile to view");

        // Update profile with the new rules.
        profile.file_rules = records;
        let src = self.src.clone();

        tokio::spawn(async move {
            if let Err(e) = src.write_profiles(vec![profile]).await {
                error!("failed to write to list: {e:?}");
            }
        });
    }

    fn clear(&mut self) {
        self.profile.take();
        self.table.clear();
    }
}

impl Component for ProfileView {
    type Context = ProfileContext;

    fn component_name(&self) -> &'static str {
        "ProfileView"
    }

    fn register_keybindings(&mut self, config: &Config) {
        // todo: this works becasue the components have no interlapping keybindings by default
        // todo: refactor this into serperate maps?
        let edit = crate::config::parse_actions(&config.keybindings["ProfileViewEdit"]);
        let view = crate::config::parse_actions(&config.keybindings[self.component_name()]);

        self.key_bindings.extend(edit);
        self.key_bindings.extend(view);
    }

    fn handle_known_event(
        &mut self,
        context: &mut Self::Context,
        event: &[crossterm::event::KeyEvent],
    ) -> Result<Option<crate::app::GlobalAction>> {
        if let Some(action) = self.key_bindings.get(event) {
            match action {
                ProfileViewActions::Edit => {
                    context.mounted = super::ProfileSubComponent::ProfileViewEdit.as_str();

                    return Ok(Some(GlobalAction::Render));
                },
                ProfileViewActions::Add => {
                    context.mounted = super::ProfileSubComponent::ProfileForm.as_str();

                    return Ok(Some(GlobalAction::Render));
                },
                ProfileViewActions::Remove => {
                    self.table.remove_selected_record();
                    return Ok(Some(GlobalAction::Render));
                },
                ProfileViewActions::Save => {
                    self.save();
                    context.mounted = super::ProfileSubComponent::ProfileView.as_str();

                    return Ok(Some(GlobalAction::Render));
                },
                ProfileViewActions::Cancel => {
                    self.restore();
                    context.mounted = super::ProfileSubComponent::ProfileView.as_str();

                    return Ok(Some(GlobalAction::Render));
                },
                ProfileViewActions::Back => {
                    self.clear();
                    context.mounted = super::ProfileSubComponent::Profiles.as_str();

                    return Ok(Some(GlobalAction::Render));
                },
                ProfileViewActions::Up => {
                    self.table.scroll_up();
                    return Ok(Some(GlobalAction::Render));
                },
                ProfileViewActions::Down => {
                    self.table.scroll_down();
                    return Ok(Some(GlobalAction::Render));
                },
                ProfileViewActions::NavLeft => {
                    self.clear();
                    return Ok(None);
                },
                ProfileViewActions::NavRight => {
                    self.clear();
                },
            }
        } else {
            log::error!("Unknown event: {:?}", event);
        }

        Ok(None)
    }

    fn is_known_event(&self, event: &[crossterm::event::KeyEvent]) -> bool {
        self.key_bindings.get(event).is_some()
    }
}

fn flatten_filter(filter: &FileRule) -> [String; COLUMN_COUNT] {
    [filter.file.display().to_string(), filter.permissions()]
}

impl Draw for ProfileView {
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

        let rows = self.table.records().map(|data| {
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
