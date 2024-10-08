use std::fs::File;
use std::io::BufReader;

use anyhow::Result;
use lightning_guard::map::{FileRule, Profile};
use ratatui::layout::{Alignment, Direction};
use ratatui::prelude::{Color, Constraint, Layout, Modifier, Rect, Style, Text};
use ratatui::widgets::{Cell, Paragraph, Row};
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;
use unicode_width::UnicodeWidthStr;

use super::{Component, Frame};
use crate::action::Action;
use crate::config::Config;
use crate::mode::Mode;
use crate::state::State;
use crate::widgets::table::Table;

const COLUMN_COUNT: usize = 2;

pub struct ProfileView {
    command_tx: Option<UnboundedSender<Action>>,
    longest_item_per_column: [u16; COLUMN_COUNT],
    table: Table<FileRule>,
    profile: Option<Profile>,
    config: Config,
    learning_mode: bool,
    log_entries: Vec<(String, String)>,
}

impl ProfileView {
    pub fn new() -> Self {
        Self {
            command_tx: None,
            longest_item_per_column: [0; COLUMN_COUNT],
            table: Table::new(),
            profile: None,
            config: Config::default(),
            learning_mode: false,
            log_entries: Vec::new(),
        }
    }

    pub fn load_profile(&mut self, profile: Profile) {
        self.table.load_records(profile.file_rules.clone());
        self.profile = Some(profile);
    }

    pub fn load_logfile(&mut self) -> Result<()> {
        // Open the logfile by adding path below before running
        let file = File::open(
            "",
        )?;
        let reader = BufReader::new(file);

        // Parse the JSON data
        let json_data: Vec<Value> = serde_json::from_reader(reader)?;

        // Clear previous entries
        self.log_entries.clear();

        // Extract target and set permission to "o" for each entry
        for entry in json_data {
            if let Some(target) = entry.get("target").and_then(|t| t.as_str()) {
                self.log_entries.push((target.to_string(), "o".to_string()));
            }
        }

        Ok(())
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
            Action::Edit => {
                self.learning_mode = false;
                Ok(Some(Action::UpdateMode(Mode::ProfileViewEdit)))
            },
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
                self.learning_mode = false;
                Ok(Some(Action::UpdateMode(Mode::Profiles)))
            },
            Action::Up => {
                if !self.learning_mode {
                    self.table.scroll_up();
                }
                Ok(Some(Action::Render))
            },
            Action::Down => {
                if !self.learning_mode {
                    self.table.scroll_down();
                }
                Ok(Some(Action::Render))
            },
            Action::NavLeft => {
                self.learning_mode = false;
                self.clear();
                Ok(None)
            },
            Action::NavRight => {
                self.learning_mode = false;
                // Todo: update this after making a "wrapping" navigator or the big refactor.
                #[cfg(feature = "logger")]
                self.clear();
                Ok(None)
            },
            Action::LearnModeToggle => {
                // Toggle learning mode on pressing "L" and load the logfile
                self.learning_mode = !self.learning_mode; // Toggle the flag
                if self.learning_mode {
                    // Load the logfile when learning mode is activated
                    self.load_logfile()?;
                }
                Ok(Some(Action::Render))
            },
            _ => Ok(None),
        }
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let chunks = Layout::default()
            .direction(Direction::Vertical) 
            .constraints(
                [
                    Constraint::Percentage(30), 
                    Constraint::Percentage(3),  
                    Constraint::Percentage(5),  
                    Constraint::Percentage(60), 
                ]
                .as_ref(),
            )
            .split(area);

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

        f.render_stateful_widget(table, chunks[0], self.table.state());

        if self.learning_mode {
            let profile_paragraph = Paragraph::new(" Learning Mode ").alignment(Alignment::Center);
            let line_length = area.width; 
            let line_text = "-".repeat(line_length as usize);

            let horizontal_line =
                Paragraph::new(line_text).style(Style::default().fg(Color::Yellow));

            f.render_widget(horizontal_line, chunks[2]); 

            f.render_widget(profile_paragraph, chunks[2]);

            let learning_mode_header =
                Row::new(vec![Cell::from("Target"), Cell::from("Permission")])
                    .style(Style::default().fg(Color::White).bg(Color::Yellow));

            let learning_mode_rows = self.log_entries.iter().map(|(target, permission)| {
                Row::new(vec![
                    Cell::from(target.as_str()),
                    Cell::from(permission.as_str()),
                ])
                .style(Style::new().fg(Color::White).bg(Color::Black))
            });

            let column_constraints = [
                Constraint::Percentage(50), 
                Constraint::Percentage(50), 
            ];

            let learning_mode_table = ratatui::widgets::Table::new(
                learning_mode_rows.collect::<Vec<_>>(), 
                &column_constraints,                    
            )
            .header(learning_mode_header)
            .highlight_style(selected_style)
            .highlight_symbol(Text::from(">"));

            f.render_widget(learning_mode_table, chunks[3]);
        }

        Ok(())
    }
}

fn flatten_filter(filter: &FileRule) -> [String; COLUMN_COUNT] {
    [filter.file.display().to_string(), filter.permissions()]
}
