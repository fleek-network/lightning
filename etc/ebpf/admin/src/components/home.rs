use std::collections::HashMap;
use std::time::Duration;

use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use indoc::indoc;
use ratatui::prelude::*;
use ratatui::widgets::*;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, Frame};
use crate::action::Action;
use crate::config::{Config, KeyBindings};

pub struct Home {
    command_tx: Option<UnboundedSender<Action>>,
    logo: String,
    config: Config,
}

impl Home {
    pub fn new() -> Self {
        let d = indoc! {"
            ▄▄▄
            █  █
            █▄▄▀

        "};
        let a = indoc! {"
             ▄▄
            █▄▄█
            █  █
        "};
        let m = indoc! {"
             ▄▄   ▄▄
             █ ▀▄▀ █
             █     █
        "};
        let n = indoc! {"
             ▄▄   ▄
             █ ▀▄ █
             █   ▀█
        "};
        let i = indoc! {"
            ▄
            █
            █
        "};

        let mut logo = a
            .lines()
            .zip(d.lines())
            .zip(m.lines())
            .zip(n.lines())
            .zip(i.lines())
            .map(|((((a, d), m), n), i)| format!("{a:5}{d:5}{m:8}{i:2}{n:4}"))
            .collect::<Vec<_>>()
            .join("\n");

        logo.push_str("\n\nUnder Construction");

        Self {
            command_tx: None,
            logo,
            config: Config::default(),
        }
    }
}

impl Component for Home {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let vchunks = Layout::vertical([
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(area);

        let hchunks = Layout::horizontal([
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(vchunks[1]);

        f.render_widget(Paragraph::new(self.logo.as_str()).centered(), hchunks[1]);
        Ok(())
    }
}
