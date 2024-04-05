use std::collections::{HashMap, HashSet};
use std::net::Ipv4Addr;
use std::time::Duration;

use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::*;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;
use unicode_width::UnicodeWidthStr;

use super::{Component, Frame};
use crate::action::Action;
use crate::config::{Config, KeyBindings};

#[derive(Default)]
pub struct FireWall {
    command_tx: Option<UnboundedSender<Action>>,
    blocklist: HashSet<Record>,
    longest_item_len: (u16, u16, u16),
    table_state: TableState,
    config: Config,
}

impl FireWall {
    pub fn new() -> Self {
        let blocklist: HashSet<Record> = vec![
            (
                "192.3.2.1".to_string(),
                "8080".to_string(),
                "tcp".to_string(),
            ),
            (
                "128.102.94.5".to_string(),
                "9888".to_string(),
                "udp".to_string(),
            ),
            (
                "88.1.94.5".to_string(),
                "9888".to_string(),
                "tcp".to_string(),
            ),
        ]
        .into_iter()
        .map(|(ip, port, proto)| Record { ip, port, proto })
        .collect();
        let longest_item_len = space_between_columns(&blocklist);
        Self {
            blocklist,
            command_tx: None,
            longest_item_len,
            table_state: TableState::default().with_selected(0),
            config: Config::default(),
        }
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

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tick => {},
            _ => {},
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let header_style = Style::default().fg(Color::White).bg(Color::Blue);
        let selected_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(Color::DarkGray);

        let header = ["IP", "Port", "Protocol"]
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
                Constraint::Min(self.longest_item_len.2),
            ],
        )
        .header(header)
        .highlight_style(selected_style)
        .highlight_symbol(Text::from(vec![bar.into()]));
        f.render_stateful_widget(t, area, &mut self.table_state);

        Ok(())
    }
}

#[derive(Eq, PartialEq, Hash)]
struct Record {
    ip: String,
    port: String,
    proto: String,
}

impl Record {
    fn flatten(&self) -> [&str; 3] {
        [self.ip.as_str(), self.port.as_str(), self.proto.as_str()]
    }
}

fn space_between_columns(items: &HashSet<Record>) -> (u16, u16, u16) {
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

    (ip_len as u16, port_len as u16, proto_len as u16)
}
