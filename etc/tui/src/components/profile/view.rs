use anyhow::Result;
use lightning_guard::map::{FileRule, Profile};
use ratatui::prelude::{Color, Constraint, Modifier, Rect, Style, Text};
use ratatui::widgets::{Cell, Row};
use tokio::sync::mpsc::UnboundedSender;
use unicode_width::UnicodeWidthStr;

use super::{Component, Frame};
use crate::action::Action;
use crate::components::utils::table::Table;
use crate::config::Config;
use crate::mode::Mode;
use crate::state::State;

const COLUMN_COUNT: usize = 2;

pub struct ProfileView {
    command_tx: Option<UnboundedSender<Action>>,
    longest_item_per_column: [u16; COLUMN_COUNT],
    table: Table<FileRule>,
    profile: Option<Profile>,
    config: Config,
}

impl ProfileView {
    pub fn new() -> Self {
        Self {
            command_tx: None,
            longest_item_per_column: [0; COLUMN_COUNT],
            table: Table::new(),
            profile: None,
            config: Config::default(),
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

    fn restore(&mut self) {
        self.table.restore_state();
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

    fn update(&mut self, action: Action, ctx: &mut State) -> Result<Option<Action>> {
        match action {
            Action::Edit => Ok(Some(Action::UpdateMode(Mode::ProfileViewEdit))),
            Action::Add => Ok(Some(Action::UpdateMode(Mode::ProfileRuleForm))),
            Action::Remove => {
                self.table.remove_selected_record();
                Ok(Some(Action::Render))
            },
            Action::Save => {
                // It is possible that the user deleted some entries thus we handle that here.
                let rules = self.table.records().cloned().collect();
                // Update state.
                ctx.update_selected_profile_rules_list(rules);
                // Commit to changes in state.
                ctx.commit_add_profiles();

                Ok(Some(Action::UpdateMode(Mode::ProfileView)))
            },
            Action::Cancel => {
                self.restore();
                Ok(Some(Action::UpdateMode(Mode::ProfileView)))
            },
            Action::UpdateMode(Mode::ProfileView) | Action::UpdateMode(Mode::ProfileViewEdit) => {
                let profile = ctx
                    .get_selected_profile()
                    .expect("A profile to have been selected");
                self.table.clear();
                self.load_profile(profile.clone());
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
            Action::NavLeft => {
                self.clear();
                Ok(None)
            },
            Action::NavRight => {
                // Todo: update this after making a "wrapping" navigator or the big refactor.
                #[cfg(feature = "logger")]
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

fn flatten_filter(filter: &FileRule) -> [String; COLUMN_COUNT] {
    [filter.file.display().to_string(), filter.permissions()]
}
