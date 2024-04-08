use std::collections::{HashMap, HashSet};
use std::net::Ipv4Addr;
use std::time::Duration;

use color_eyre::eyre::Result;
use color_eyre::owo_colors::OwoColorize;
use color_eyre::Report;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
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

#[derive(Default)]
pub struct FireWall {
    command_tx: Option<UnboundedSender<Action>>,
    blocklist: HashSet<Record>,
    // Table widget.
    longest_item_len: (u16, u16, u16, u16),
    table_state: TableState,
    // Input widgets.
    show_new_entry_popup: bool,
    text_areas: Vec<InputField>,
    selected_text_area: usize,
    config: Config,
}

impl FireWall {
    pub fn new() -> Self {
        let mut text_areas: Vec<_> =
            vec![("IP", TextArea::default()), ("Port", TextArea::default())]
                .into_iter()
                .map(|(title, area)| InputField { title, area })
                .collect();
        activate(&mut text_areas[0]);
        inactivate(&mut text_areas[1]);

        Self {
            blocklist: HashSet::new(),
            command_tx: None,
            longest_item_len: (0, 0, 0, 0),
            table_state: TableState::default().with_selected(0),
            config: Config::default(),
            show_new_entry_popup: false,
            text_areas,
            selected_text_area: 0,
        }
    }

    fn selected_field(&mut self) -> &mut InputField {
        &mut self.text_areas[self.selected_text_area]
    }

    fn process_input(&mut self) -> Result<()> {
        for field in self.text_areas.iter_mut() {
            field.area.select_all();
            field.area.cut();
        }

        let ip: Ipv4Addr = self.text_areas[0].area.yank_text().trim().parse()?;
        let port: u16 = self.text_areas[1].area.yank_text().trim().parse()?;
        let rec = Record {
            ip: ip.to_string(),
            port: port.to_string(),
            proto: "tcp".to_string(),
            author: "8889".to_string(),
        };
        self.blocklist.insert(rec);
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
        if self.show_new_entry_popup {
            self.selected_field().area.input(Input::from(key));
        }
        Ok(None)
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Save => {
                if let Err(e) = self.process_input() {
                    // Todo: display error.
                } else {
                    self.show_new_entry_popup = false;
                }
                Ok(Some(Action::UpdateMode(Mode::Firewall)))
            },
            Action::Cancel => {
                self.show_new_entry_popup = false;
                let _ = self.process_input();
                Ok(Some(Action::UpdateMode(Mode::Firewall)))
            },
            Action::Add => {
                self.show_new_entry_popup = true;
                Ok(Some(Action::UpdateMode(Mode::FirewallNewEntry)))
            },
            Action::Up => {
                if self.show_new_entry_popup {
                    if self.selected_text_area > 0 {
                        self.selected_text_area -= 1;
                    }
                } else {
                    // Scroll up blocklist.
                }
                Ok(Some(Action::Render))
            },
            Action::Down => {
                if self.show_new_entry_popup {
                    if self.selected_text_area < self.text_areas.len() - 1 {
                        self.selected_text_area += 1;
                    }
                } else {
                    // Scroll up blocklist.
                }
                Ok(Some(Action::Render))
            },
            _ => Ok(None),
        }
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        self.longest_item_len = space_between_columns(&self.blocklist);

        let header_style = Style::default().fg(Color::White).bg(Color::Blue);
        let selected_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(Color::DarkGray);

        let header = ["IP", "Port", "Protocol", "Pid"]
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style)
            .height(1);

        let rows = self.blocklist.iter().enumerate().map(|(i, data)| {
            let item = data.flatten();
            item.into_iter()
                .map(|content| {
                    let text = Text::from(format!("{content}"));
                    Cell::from(text)
                })
                .collect::<Row>()
                .style(Style::new().fg(Color::White).bg(Color::Black))
                .height(1)
        });

        let bar = " > ";
        let t = Table::new(
            rows,
            [
                Constraint::Min(self.longest_item_len.0 + 1),
                Constraint::Min(self.longest_item_len.1 + 1),
                Constraint::Min(self.longest_item_len.2 + 1),
                Constraint::Min(self.longest_item_len.3),
            ],
        )
        .header(header)
        .highlight_style(selected_style)
        .highlight_symbol(Text::from(vec![bar.into()]));
        f.render_stateful_widget(t, area, &mut self.table_state);

        if self.show_new_entry_popup {
            let block = Block::default()
                .title("Block")
                .borders(Borders::ALL)
                .border_style(Style::from(Color::Red));
            f.render_widget(Clear, area);
            let area = centered_form(20, 40, area);
            f.render_widget(block, area);

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(area);
            for (i, (textarea, chunk)) in self.text_areas.iter_mut().zip(chunks.iter()).enumerate()
            {
                if i == self.selected_text_area {
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

#[derive(Eq, PartialEq, Hash)]
struct Record {
    ip: String,
    port: String,
    proto: String,
    author: String,
}

impl Record {
    fn flatten(&self) -> [&str; 4] {
        [
            self.ip.as_str(),
            self.port.as_str(),
            self.proto.as_str(),
            self.author.as_str(),
        ]
    }
}

fn inactivate(field: &mut InputField) {
    field.area.set_cursor_line_style(Style::default());
    field.area.set_cursor_style(Style::default());
    field.area.set_block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::DarkGray))
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
            .style(Style::default())
            .title(field.title),
    );
}

fn centered_form(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

fn space_between_columns(items: &HashSet<Record>) -> (u16, u16, u16, u16) {
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

    (
        ip_len as u16,
        port_len as u16,
        proto_len as u16,
        author_len as u16,
    )
}

struct InputField {
    title: &'static str,
    area: TextArea<'static>,
}
