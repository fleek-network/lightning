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
use tokio::runtime::Handle;
use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::{Input, TextArea};
use unicode_width::UnicodeWidthStr;

use super::{Component, Frame};
use crate::action::Action;
use crate::config::{Config, KeyBindings};
use crate::mode::Mode;

const IP_FIELD_NAME: &str = "IP";
const PORT_FIELD_NAME: &str = "Port";
const COLUMN_COUNT: usize = 6;
const INPUT_FORM_X: u16 = 20;
const INPUT_FORM_Y: u16 = 40;
const INPUT_FIELD_COUNT: usize = 2;

#[derive(Default)]
pub struct FireWall {
    command_tx: Option<UnboundedSender<Action>>,
    filters: Vec<PacketFilterRule>,
    storage: Storage,
    // Table widget for displaying records.
    longest_item_per_column: [u16; COLUMN_COUNT],
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

    pub async fn read_state_from_storage(&mut self) -> Result<()> {
        // If it's an error, there is no file and thus there is nothing to do.
        if let Ok(filters) = self
            .storage
            .read_packet_filters()
            .await
            .map_err(|e| Report::msg(e.to_string()))
        {
            self.filters = filters.into_iter().map(Into::into).collect();
        }

        Ok(())
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

    fn remove_filter(&mut self) {
        if let Some(cur) = self.table_state.selected() {
            debug_assert!(cur < self.filters.len());
            self.filters.remove(cur);

            if self.filters.is_empty() {
                self.table_state.select(None);
            } else if cur == self.filters.len() {
                self.table_state.select(Some(cur - 1));
            } else if cur == 0 {
                self.table_state.select(Some(1));
            }
        }
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
        let filter = PacketFilterRule {
            prefix: PacketFilterRule::DEFAULT_PREFIX,
            ip,
            port,
            shortlived: false,
            // Todo: get these from input.
            proto: PacketFilterRule::TCP,
            trigger_event: true,
            action: PacketFilterRule::DROP,
        };

        self.filters.push(filter);
        self.update_storage();
        Ok(())
    }

    fn update_storage(&self) {
        let command_tx = self
            .command_tx
            .clone()
            .expect("Component always has a sender");
        let storage = self.storage.clone();
        let new = self.filters.clone().into_iter().collect::<Vec<_>>();
        tokio::spawn(async move {
            if let Err(e) = storage.write_packet_filters(new).await {
                let _ = command_tx.send(Action::Error(e.to_string()));
            }
        });
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
                if let Err(e) = self.update_filters_from_input() {
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
            Action::Remove => {
                self.remove_filter();
                self.update_storage();
                Ok(Some(Action::Render))
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
        if !self.show_input_field {
            // Display list of filter rules.
            self.longest_item_per_column = space_between_columns(&self.filters);
            debug_assert!(self.longest_item_per_column.len() == COLUMN_COUNT);

            let column_names = [
                "IP",
                "Subnet",
                "Port",
                "Protocol",
                "Trigger Event",
                "Action",
            ];
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

            let rows = self.filters.iter().enumerate().map(|(i, data)| {
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
                Constraint::Min(self.longest_item_per_column[1] + 1),
                Constraint::Min(self.longest_item_per_column[2] + 1),
                Constraint::Min(self.longest_item_per_column[3] + 1),
                Constraint::Min(self.longest_item_per_column[4] + 1),
                Constraint::Min(self.longest_item_per_column[5]),
            ];
            debug_assert!(contraints.len() == COLUMN_COUNT);

            let bar = " > ";
            let table = Table::new(rows, contraints)
                .header(header)
                .highlight_style(selected_style)
                .highlight_symbol(Text::from(bar));

            f.render_stateful_widget(table, area, &mut self.table_state);
        } else {
            // Display form to enter new rule.
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

fn space_between_columns(items: &Vec<PacketFilterRule>) -> [u16; COLUMN_COUNT] {
    let prefix = items
        .iter()
        .map(|r| r.prefix.to_string().as_str().width())
        .max()
        .unwrap_or(0);
    let ip_len = items
        .iter()
        .map(|r| r.ip.to_string().as_str().width())
        .max()
        .unwrap_or(0);
    let port_len = items
        .iter()
        .map(|r| r.port.to_string().as_str().width())
        .max()
        .unwrap_or(0);
    let proto_len = items
        .iter()
        .map(|r| r.proto_str().as_str().width())
        .max()
        .unwrap_or(0);
    let trigger_event_len = items
        .iter()
        .map(|r| r.trigger_event.to_string().as_str().width())
        .max()
        .unwrap_or(0);
    let action_len = items
        .iter()
        .map(|r| r.action_str().as_str().width())
        .max()
        .unwrap_or(0);

    [
        ip_len as u16,
        prefix as u16,
        port_len as u16,
        proto_len as u16,
        trigger_event_len as u16,
        action_len as u16,
    ]
}

fn flatten_filter(filter: &PacketFilterRule) -> [String; COLUMN_COUNT] {
    [
        filter.ip.to_string(),
        filter.prefix.to_string(),
        filter.port.to_string(),
        filter.proto_str(),
        filter.trigger_event.to_string(),
        filter.action_str(),
    ]
}
