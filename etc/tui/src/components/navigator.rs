use anyhow::Result;
use ratatui::prelude::{Rect, Style};
use ratatui::style::Stylize;
use ratatui::widgets::{Block, Borders, Tabs};
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, Frame};
use crate::action::Action;
use crate::config::Config;
use crate::state::State;

/// Component for switching between tabs.
#[derive(Default)]
pub struct Navigator {
    command_tx: Option<UnboundedSender<Action>>,
    selected_tab: usize,
    tabs: Vec<&'static str>,
    config: Config,
}

impl Navigator {
    pub fn new() -> Self {
        let tabs = vec![
            "Home",
            "Firewall",
            "Profiles",
            "Summary",
            #[cfg(feature = "logger")]
            "Logger",
        ];
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

    fn update(&mut self, action: Action, _: &mut State) -> Result<Option<Action>> {
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
            .select(self.selected_tab)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().white())
            .highlight_style(Style::default().yellow())
            .padding(" ", " ");
        f.render_widget(t, area);
        Ok(())
    }
}
