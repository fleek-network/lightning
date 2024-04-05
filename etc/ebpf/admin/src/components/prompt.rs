use std::collections::HashMap;
use std::time::Duration;

use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::block::Title;
use ratatui::widgets::*;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, Frame};
use crate::action::Action;
use crate::config::{Config, KeyBindings};

#[derive(Default)]
pub struct Prompt {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
}

impl Prompt {
    pub fn new() -> Self {
        Self::default()
    }

    fn draw_tree(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        f.render_widget(Block::default().style(Style::default()), area);

        let chunks = Layout::default()
            .vertical_margin(1)
            .horizontal_margin(1)
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(1)].as_ref())
            .split(area);

        Ok(())
    }
}

impl Component for Prompt {
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
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)].as_ref())
            .split(area);

        let p = Paragraph::default()
            .block(
                Block::default()
                    .title("0 notifications")
                    // .title(Title::from("Center").alignment(Alignment::Center))
                    .title_alignment(Alignment::Center),
            )
            .style(Style::default().fg(Color::White))
            .centered();
        f.render_widget(p, area);
        Ok(())
    }
}
