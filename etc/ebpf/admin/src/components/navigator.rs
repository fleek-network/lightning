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
use crate::mode::Mode;

#[derive(Default)]
pub struct Navigator {
    command_tx: Option<UnboundedSender<Action>>,
    selected_tab: usize,
    tabs: Vec<&'static str>,
    config: Config,
}

impl Navigator {
    pub fn new() -> Self {
        let tabs = vec!["Home", "Firewall", "Access"];
        Self {
            tabs,
            command_tx: None,
            selected_tab: 0,
            config: Config::default(),
        }
    }
}

impl Component for Navigator {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        let last_selected_tab = self.selected_tab;
        match action {
            Action::NavLeft => {
                if self.selected_tab > 0 {
                    self.selected_tab -= 1;
                }
            },
            Action::NavRight => {
                if self.selected_tab < self.tabs.len() - 1 {
                    self.selected_tab += 1;
                }
            },
            _ => {},
        }
        if last_selected_tab != self.selected_tab {
            Ok(Some(Action::UpdateMode(
                self.tabs[self.selected_tab]
                    .parse()
                    .expect("Modes are hardcoded"),
            )))
        } else {
            Ok(None)
        }
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let t = Tabs::new(self.tabs.clone())
            .select(self.selected_tab as usize)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().white())
            .highlight_style(Style::default().yellow())
            .padding(" ", " ");
        f.render_widget(t, area);
        Ok(())
    }
}
