pub mod form;

use color_eyre::eyre::Result;
use color_eyre::Report;
use ebpf_service::map::PacketFilterRule;
use ebpf_service::ConfigSource;
use ratatui::prelude::{Color, Constraint, Modifier, Rect, Style, Text};
use ratatui::widgets::{Cell, Row, TableState};
use tokio::sync::mpsc::UnboundedSender;
use unicode_width::UnicodeWidthStr;

use super::{Component, Frame};
use crate::action::Action;
use crate::components::firewall::form::FirewallForm;
use crate::config::{Config, KeyBindings};
use crate::mode::Mode;
use crate::widgets::table::Table;

const COLUMN_COUNT: usize = 6;

/// Component that displaying and managing packet-filter rules.
pub struct FireWall {
    command_tx: Option<UnboundedSender<Action>>,
    table: Table<PacketFilterRule>,
    longest_item_per_column: [u16; COLUMN_COUNT],
    form: FirewallForm,
    src: ConfigSource,
    config: Config,
}

impl FireWall {
    pub fn new(src: ConfigSource) -> Self {
        Self {
            src,
            table: Table::new(),
            command_tx: None,
            longest_item_per_column: [0; COLUMN_COUNT],
            form: FirewallForm::new(),
            config: Config::default(),
        }
    }

    pub async fn read_state_from_storage(&mut self) -> Result<()> {
        // If it's an error, there is no file and thus there is nothing to do.
        if let Ok(filters) = self
            .src
            .read_packet_filters()
            .await
            .map_err(|e| Report::msg(e.to_string()))
        {
            self.table.load_records(filters);
        }

        Ok(())
    }

    pub fn form(&mut self) -> &mut FirewallForm {
        &mut self.form
    }

    fn save(&mut self) {
        self.table.commit_changes();
        let command_tx = self
            .command_tx
            .clone()
            .expect("Component always has a sender");
        let storage = self.src.clone();
        let new = self.table.records().cloned().collect::<Vec<_>>();
        tokio::spawn(async move {
            if let Err(e) = storage.write_packet_filters(new).await {
                let _ = command_tx.send(Action::Error(e.to_string()));
            }
        });
    }

    fn space_between_columns(&self) -> [u16; COLUMN_COUNT] {
        let prefix = self
            .table
            .records()
            .map(|r| r.prefix.to_string().as_str().width())
            .max()
            .unwrap_or(0);
        let ip_len = self
            .table
            .records()
            .map(|r| r.ip.to_string().as_str().width())
            .max()
            .unwrap_or(0);
        let port_len = self
            .table
            .records()
            .map(|r| r.port.to_string().as_str().width())
            .max()
            .unwrap_or(0);
        let proto_len = self
            .table
            .records()
            .map(|r| r.proto_str().as_str().width())
            .max()
            .unwrap_or(0);
        let trigger_event_len = self
            .table
            .records()
            .map(|r| r.audit.to_string().as_str().width())
            .max()
            .unwrap_or(0);
        let action_len = self
            .table
            .records()
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
            Action::Edit => Ok(Some(Action::UpdateMode(Mode::FirewallEdit))),
            Action::Add => Ok(Some(Action::UpdateMode(Mode::FirewallForm))),
            Action::Save => {
                self.save();
                Ok(Some(Action::UpdateMode(Mode::Firewall)))
            },
            Action::Cancel => {
                self.table.restore_state();
                Ok(Some(Action::UpdateMode(Mode::Firewall)))
            },
            Action::Remove => {
                self.table.remove_selected_record();
                Ok(Some(Action::Render))
            },
            Action::Up => {
                self.table.scroll_up();
                Ok(Some(Action::Render))
            },
            Action::Down => {
                self.table.scroll_down();
                Ok(Some(Action::Render))
            },
            Action::UpdateMode(Mode::FirewallEdit) => {
                // It's possible that the form sent this so we try to yank a new input value.
                if let Some(rule) = self.form.yank_input() {
                    self.table.add_record(rule);
                }
                Ok(None)
            },
            _ => Ok(None),
        }
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        self.longest_item_per_column = self.space_between_columns();
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

        let rows = self.table.records().enumerate().map(|(i, data)| {
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
        let table = ratatui::widgets::Table::new(rows, contraints)
            .header(header)
            .highlight_style(selected_style)
            .highlight_symbol(Text::from(bar));

        f.render_stateful_widget(table, area, self.table.state());

        Ok(())
    }
}

fn flatten_filter(filter: &PacketFilterRule) -> [String; COLUMN_COUNT] {
    [
        filter.ip.to_string(),
        filter.prefix.to_string(),
        filter.port.to_string(),
        filter.proto_str(),
        filter.audit.to_string(),
        filter.action_str(),
    ]
}
