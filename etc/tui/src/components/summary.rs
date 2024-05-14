use color_eyre::eyre::Result;
use ratatui::layout::Rect;
use ratatui::prelude::{Alignment, Color, Constraint, Layout, Style, Stylize, Text};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Paragraph, Row};
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, Frame};
use crate::action::Action;
use crate::config::Config;
use crate::widgets::table::Table;

/// Component for displaying summary statistics and event notification.
#[derive(Default)]
pub struct Summary {
    command_tx: Option<UnboundedSender<Action>>,
    process_metrics: Table<ProcessMetrics>,
    network_metrics: Table<NetworkMetrics>,
    config: Config,
}

impl Summary {
    pub fn new() -> Self {
        let proc_mock_metrics = vec![
            ProcessMetrics {
                name: "JS".to_string(),
                cpu: 1.2,
                mem: 3.3,
            },
            ProcessMetrics {
                name: "AI".to_string(),
                cpu: 1.1,
                mem: 5.2,
            },
            ProcessMetrics {
                name: "Fetcher".to_string(),
                cpu: 0.8,
                mem: 1.3,
            },
            ProcessMetrics {
                name: "Core".to_string(),
                cpu: 3.2,
                mem: 5.8,
            },
            ProcessMetrics {
                name: "eBPF".to_string(),
                cpu: 0.1,
                mem: 0.3,
            },
            ProcessMetrics {
                name: "SvcX".to_string(),
                cpu: 1.2,
                mem: 3.3,
            },
        ];
        let mut process_metrics = Table::new();
        process_metrics.load_records(proc_mock_metrics);

        let net_mock_metrics = vec![
            NetworkMetrics {
                name: "Pool".to_string(),
                port: 8839,
                tx: 300,
                rx: 700,
            },
            NetworkMetrics {
                name: "Handshake".to_string(),
                port: 8881,
                tx: 300,
                rx: 700,
            },
            NetworkMetrics {
                name: "RPC".to_string(),
                port: 8870,
                tx: 300,
                rx: 700,
            },
        ];
        let mut network_metrics = Table::new();
        network_metrics.load_records(net_mock_metrics);

        Self {
            process_metrics,
            network_metrics,
            ..Default::default()
        }
    }

    fn draw_config(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let profiles = ratatui::widgets::List::new([
            Text::from("Rate Limiter: 10,000ps").left_aligned(),
            Text::from("Audit: false").left_aligned(),
            Text::from("Email: enabled").left_aligned(),
            Text::from("Text: disabled").left_aligned(),
            Text::from("Text: disabled").left_aligned(),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title_alignment(Alignment::Center)
                .title("Configuration"),
        );
        f.render_widget(profiles, area);
        Ok(())
    }

    fn draw_networking_metrics(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let column_names = ["Name", "Port", "Tx", "Rx"];
        let header_style = Style::default().fg(Color::White).reversed();
        let header = column_names
            .into_iter()
            .map(|name| Cell::from(Text::from(name).centered()))
            .collect::<Row>()
            .style(header_style)
            .top_margin(1);

        let rows = self.network_metrics.records().enumerate().map(|(i, data)| {
            let item = data.flatten_filter();
            let bg = if i % 2 == 1 {
                Color::Black
            } else {
                Color::DarkGray
            };
            item.into_iter()
                .map(|content| {
                    let text = Text::from(content).centered();
                    Cell::from(text)
                })
                .collect::<Row>()
                .style(Style::new().fg(Color::White).bg(bg))
                .height(1)
        });

        let contraints = [
            Constraint::Min(5 + 1),
            Constraint::Min(5 + 1),
            Constraint::Min(5 + 1),
            Constraint::Min(5),
        ];

        let table = ratatui::widgets::Table::new(rows, contraints).header(header);
        f.render_stateful_widget(table, area, self.network_metrics.state());

        Ok(())
    }

    fn draw_process_metrics(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let column_names = ["Process", "CPU%", "MEM%"];
        let header_style = Style::default().fg(Color::White).reversed();
        let header = column_names
            .into_iter()
            .map(|name| Cell::from(Text::from(name).centered()))
            .collect::<Row>()
            .style(header_style)
            .top_margin(1);

        let rows = self.process_metrics.records().enumerate().map(|(i, data)| {
            let item = data.flatten_filter();
            let bg = if i % 2 == 1 {
                Color::Black
            } else {
                Color::DarkGray
            };
            item.into_iter()
                .map(|content| {
                    let text = Text::from(content).centered();
                    Cell::from(text)
                })
                .collect::<Row>()
                .style(Style::new().fg(Color::White).bg(bg))
                .height(1)
        });

        let contraints = [
            Constraint::Min(5 + 1),
            Constraint::Min(5 + 1),
            Constraint::Min(5),
        ];
        let table = ratatui::widgets::Table::new(rows, contraints).header(header);
        f.render_stateful_widget(table, area, self.network_metrics.state());

        Ok(())
    }
}

impl Component for Summary {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let overview = Paragraph::new("Overview\n")
            .block(
                Block::bordered()
                    .title_alignment(Alignment::Center)
                    .border_type(BorderType::Rounded),
            )
            .style(Style::default().fg(Color::White))
            .centered();
        f.render_widget(overview, area);

        let content = Layout::default()
            .vertical_margin(3)
            .horizontal_margin(3)
            .constraints([Constraint::Percentage(100)])
            .split(area);

        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Max(8),
            Constraint::Length(1),
            Constraint::Max(6),
            Constraint::Length(1),
            Constraint::Max(6),
            Constraint::Fill(1),
        ])
        .split(content[0]);
        f.render_widget(
            Paragraph::new("No notifications")
                .alignment(Alignment::Left)
                .style(Style::default().fg(Color::White).reversed())
                .centered(),
            chunks[0],
        );
        self.draw_process_metrics(f, chunks[2])?;
        self.draw_networking_metrics(f, chunks[4])?;
        self.draw_config(f, chunks[6])?;

        Ok(())
    }
}

#[derive(Default)]
pub struct ProcessMetrics {
    name: String,
    cpu: f32,
    mem: f32,
}

impl ProcessMetrics {
    fn flatten_filter(&self) -> [String; 3] {
        [
            self.name.clone(),
            self.cpu.to_string(),
            self.mem.to_string(),
        ]
    }
}

#[derive(Default)]
pub struct NetworkMetrics {
    name: String,
    port: u16,
    tx: usize,
    rx: usize,
}

impl NetworkMetrics {
    fn flatten_filter(&self) -> [String; 4] {
        [
            self.name.clone(),
            self.port.to_string(),
            self.tx.to_string(),
            self.rx.to_string(),
        ]
    }
}
