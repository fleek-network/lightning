use std::collections::HashMap;
use std::time::Duration;

use color_eyre::eyre::Result;
use indoc::indoc;
use ratatui::prelude::{Constraint, Layout, Rect};
use ratatui::widgets::Paragraph;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, Frame};
use crate::action::Action;
use crate::config::Config;

/// Component that displaying the home page.
#[derive(Default)]
pub struct Home {
    command_tx: Option<UnboundedSender<Action>>,
    title: String,
    logo: String,
    config: Config,
}

impl Home {
    pub fn new() -> Self {
        let logo = indoc! {"
                  ▄
               ▄█▀
           ▄▄██▀
        ▄██████████▀
             ▄██▀
           ▄█▀
          ▀
       "};

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

        let mut title = a
            .lines()
            .zip(d.lines())
            .zip(m.lines())
            .zip(n.lines())
            .zip(i.lines())
            .map(|((((a, d), m), n), i)| format!("{a:5}{d:5}{m:8}{i:2}{n:4}"))
            .collect::<Vec<_>>()
            .join("\n");
        title.push_str("\n\nUnder Construction");

        let logo = logo
            .lines()
            .map(|a| format!("{a:12}"))
            .collect::<Vec<_>>()
            .join("\n");

        Self {
            command_tx: None,
            logo,
            title,
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
            Constraint::Fill(1),
            Constraint::Max(42),
            Constraint::Fill(1),
        ])
        .split(vchunks[1]);

        let logo =
            Layout::horizontal([Constraint::Length(12), Constraint::Fill(1)]).split(hchunks[1]);

        f.render_widget(Paragraph::new(self.logo.as_str()).right_aligned(), logo[0]);

        let title = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).split(logo[1]);
        f.render_widget(Paragraph::new(self.title.as_str()).centered(), title[1]);

        Ok(())
    }
}
