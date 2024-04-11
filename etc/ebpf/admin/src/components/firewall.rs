use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

use color_eyre::eyre::Result;
use color_eyre::owo_colors::OwoColorize;
use color_eyre::Report;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ebpf_service::map::storage::Storage;
use ebpf_service::map::PacketFilterRule;
use log::error;
use ratatui::prelude::*;
use ratatui::widgets::*;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::{Input, TextArea};
use unicode_width::UnicodeWidthStr;

use super::{Component, Frame};
use crate::action::Action;
use crate::config::{Config, KeyBindings};
use crate::mode::Mode;

const IP_FIELD_NAME: &str = "IP";
const PORT_FIELD_NAME: &str = "Port";
const COLUMN_COUNT: usize = 4;
const INPUT_FORM_X: u16 = 20;
const INPUT_FORM_Y: u16 = 40;
const INPUT_FIELD_COUNT: usize = 2;

#[derive(Default)]
pub struct FireWall {
    command_tx: Option<UnboundedSender<Action>>,
    filters: Vec<Filter>,
    storage: Storage,
    // Table widget for displaying records.
    longest_item_per_column: [u16; 4],
    table_state: TableState,
    // Input widgets for adding a new record.
    show_input_field: bool,
    input_fields: Vec<InputField>,
    selected_input_field: usize,
    config: Config,
}

impl FireWall {
    pub fn new(storage: Storage) -> Self {
        let mut input_fields: Vec<_> = vec![
            (IP_FIELD_NAME, TextArea::default()),
            (PORT_FIELD_NAME, TextArea::default()),
        ]
        .into_iter()
        .map(|(title, area)| InputField { title, area })
        .collect();

        debug_assert!(input_fields.len() == INPUT_FIELD_COUNT);
        activate(&mut input_fields[0]);
        inactivate(&mut input_fields[1]);

        Self {
            filters: Vec::new(),
            storage,
            command_tx: None,
            longest_item_per_column: [0; COLUMN_COUNT],
            table_state: TableState::default().with_selected(0),
            config: Config::default(),
            show_input_field: false,
            input_fields,
            selected_input_field: 0,
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

    fn process_input(&mut self) -> Result<()> {
        for field in self.input_fields.iter_mut() {
            field.area.select_all();
            field.area.cut();
        }

        let ip: Ipv4Addr = self.input_fields[0]
            .area
            .yank_text()
            .trim()
            .parse()
            .map_err(|_| Report::msg("Invalid IP"))?;
        let port: u16 = self.input_fields[1]
            .area
            .yank_text()
            .trim()
            .parse()
            .map_err(|_| Report::msg("Invalid port"))?;
        let filter = Filter {
            ip: ip.to_string(),
            port: port.to_string(),
            // Todo: update.
            proto: "tcp".to_string(),
            author: "100".to_string(),
        };

        self.filters.push(filter);

        let command_tx = self
            .command_tx
            .clone()
            .expect("Component always has a sender");
        let storage = self.storage.clone();
        let new = self
            .filters
            .clone()
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>();
        tokio::spawn(async move {
            if let Err(e) = storage.write_packet_filters(new).await {
                let _ = command_tx.send(Action::Error(e.to_string()));
            }
        });
        Ok(())
    }
}

impl Component for FireWall {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        if self.show_input_field {
            self.selected_field().area.input(Input::from(key));
        }
        Ok(None)
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Save => {
                if let Err(e) = self.process_input() {
                    return Ok(Some(Action::Error(e.to_string())));
                } else {
                    self.show_input_field = false;
                }
                Ok(Some(Action::UpdateMode(Mode::Firewall)))
            },
            Action::Cancel => {
                self.show_input_field = false;
                self.clear_input();
                Ok(Some(Action::UpdateMode(Mode::Firewall)))
            },
            Action::Add => {
                self.show_input_field = true;
                Ok(Some(Action::UpdateMode(Mode::FirewallNewEntry)))
            },
            Action::Up => {
                if self.show_input_field {
                    if self.selected_input_field > 0 {
                        self.selected_input_field -= 1;
                    }
                } else {
                    // Scroll up blocklist.
                    self.scroll_up();
                }
                Ok(Some(Action::Render))
            },
            Action::Down => {
                if self.show_input_field {
                    if self.selected_input_field < self.input_fields.len() - 1 {
                        self.selected_input_field += 1;
                    }
                } else {
                    // Scroll down blocklist.
                    self.scroll_down();
                }
                Ok(Some(Action::Render))
            },
            _ => Ok(None),
        }
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        self.longest_item_per_column = space_between_columns(&self.filters);

        let header_style = Style::default().fg(Color::White).bg(Color::Blue);
        let selected_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(Color::DarkGray);
        let header = ["IP", "Port", "Protocol", "Pid"]
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style);

        let rows = self.filters.iter().enumerate().map(|(i, data)| {
            let item = data.flatten();
            item.into_iter()
                .map(|content| {
                    let text = Text::from(format!("{content}"));
                    Cell::from(text)
                })
                .collect::<Row>()
                .style(Style::new().fg(Color::White).bg(Color::Black))
        });

        debug_assert!(self.longest_item_per_column.len() == COLUMN_COUNT);
        let bar = " > ";
        let table = Table::new(
            rows,
            [
                Constraint::Min(self.longest_item_per_column[0] + 1),
                Constraint::Min(self.longest_item_per_column[1] + 1),
                Constraint::Min(self.longest_item_per_column[2] + 1),
                Constraint::Min(self.longest_item_per_column[3]),
            ],
        )
        .header(header)
        .highlight_style(selected_style)
        .highlight_symbol(Text::from(bar));
        f.render_stateful_widget(table, area, &mut self.table_state);

        if self.show_input_field {
            debug_assert!(self.input_fields.len() == INPUT_FIELD_COUNT);

            f.render_widget(Clear, area);
            let area = center_form(INPUT_FORM_X, INPUT_FORM_Y, area);

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    [
                        Constraint::Percentage(0),
                        Constraint::Max(3),
                        Constraint::Max(3),
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
        }

        Ok(())
    }
}

struct InputField {
    title: &'static str,
    area: TextArea<'static>,
}

#[derive(Clone, Eq, PartialEq, Hash)]
struct Filter {
    ip: String,
    port: String,
    proto: String,
    author: String,
}

impl Filter {
    fn flatten(&self) -> [&str; 4] {
        [
            self.ip.as_str(),
            self.port.as_str(),
            self.proto.as_str(),
            self.author.as_str(),
        ]
    }
}

impl From<PacketFilterRule> for Filter {
    fn from(value: PacketFilterRule) -> Self {
        Filter {
            ip: value.ip.to_string(),
            port: value.port.to_string(),
            proto: "tcp".to_string(),
            author: "888".to_string(),
        }
    }
}

impl From<Filter> for PacketFilterRule {
    fn from(value: Filter) -> Self {
        PacketFilterRule {
            ip: value.ip.parse().unwrap(),
            port: value.port.parse().unwrap(),
        }
    }
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

fn space_between_columns(items: &Vec<Filter>) -> [u16; 4] {
    let ip_len = items
        .iter()
        .map(|r| r.ip.as_str())
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);
    let port_len = items
        .iter()
        .map(|r| r.port.as_str())
        .flat_map(str::lines)
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);
    let proto_len = items
        .iter()
        .map(|r| r.proto.as_str())
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);
    let author_len = items
        .iter()
        .map(|r| r.proto.as_str())
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);

    [
        ip_len as u16,
        port_len as u16,
        proto_len as u16,
        author_len as u16,
    ]
}
