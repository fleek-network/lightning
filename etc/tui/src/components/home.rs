use anyhow::Result;
use indoc::indoc;
use ratatui::prelude::{Constraint, Layout, Rect};
use ratatui::widgets::Paragraph;
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, Draw, Frame};
use crate::app::{AppAction, ApplicationContext};
use crate::config::{ComponentKeyBindings, Config};

/// Component that displaying the home page.
#[derive(Default)]
pub struct Home {
    title: String,
    logo: String,
    config: Config,
    keybindings: ComponentKeyBindings<AppAction>
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
            logo,
            title,
            keybindings: Default::default(),
            config: Config::default(),
        }
    }
}

impl Draw for Home {
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

impl Component for Home {
    type Context = ApplicationContext;

    fn component_name(&self) -> &'static str {
        "Home"
    }

    fn handle_known_event(
        &mut self,
        context: &mut Self::Context,
        event: &[crossterm::event::KeyEvent],
    ) -> Result<Option<crate::app::GlobalAction>> {
        if let Some(action) = self.keybindings.get(event) {
            return Ok(context.handle_app_action(*action))
        } else {
            log::error!("Unknown event: {:?}", event);
            log::error!("This should have been prevalidated before being passed to the component.")
        }

        Ok(None)
    }

    fn is_known_event(&self, event: &[crossterm::event::KeyEvent]) -> bool {
        self.keybindings.get(event).is_some()
    }

    fn register_keybindings(&mut self, config: &Config) {
        self.keybindings = crate::config::parse_actions(&config.keybindings[self.component_name()])
    }
}
