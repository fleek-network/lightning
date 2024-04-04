use std::collections::HashMap;
use std::time::Duration;

use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::*;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, Frame};
use crate::action::Action;
use crate::config::{Config, KeyBindings};

#[derive(Default)]
pub struct SideBar {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
}

impl SideBar {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Component for SideBar {
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
        let p = Paragraph::new("Security\n")
            .block(
                Block::bordered()
                    .title_alignment(Alignment::Center)
                    .border_type(BorderType::Rounded),
            )
            .style(Style::default().fg(Color::White))
            .centered();
        f.render_widget(p, area);
        Ok(())
    }
}
