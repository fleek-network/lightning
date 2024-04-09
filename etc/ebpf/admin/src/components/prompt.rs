use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use color_eyre::eyre::Result;
use color_eyre::Report;
use colored::Colorize;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Flex;
use ratatui::prelude::*;
use ratatui::widgets::block::Title;
use ratatui::widgets::*;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, Frame};
use crate::action::Action;
use crate::config::{Config, KeyBindings};
use crate::mode::Mode;

#[derive(Default)]
pub struct Prompt {
    command_tx: Option<UnboundedSender<Action>>,
    current: HashMap<KeySymbol, Action>,
    config: Config,
}

impl Prompt {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_state(&mut self, mode: Mode) {
        if let Some(keys) = self.config.keybindings.get(&mode) {
            let keys = keys.clone();
            let codes = keys
                .into_iter()
                .map(|(key, value)| {
                    debug_assert!(key.len() == 1);
                    (KeySymbol::from(key[0]), value)
                })
                .collect::<HashMap<_, _>>();
            self.current = codes;
        }
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

    fn update(&mut self, _action: Action) -> Result<Option<Action>> {
        Ok(None)
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(&[
                Constraint::Percentage(10),
                Constraint::Percentage(45),
                Constraint::Percentage(45),
            ])
            .split(area);

        let contraints = vec![
            Constraint::Fill(1),
            Constraint::Fill(1),
            Constraint::Fill(1),
            Constraint::Fill(1),
            Constraint::Fill(1),
            Constraint::Fill(1),
        ];

        let first_row = Layout::default()
            .direction(Direction::Horizontal)
            .constraints::<&Vec<_>>(contraints.as_ref())
            .split(rows[1]);

        for (i, (key, action)) in self.current.iter().take(6).enumerate() {
            // Todo: Remove unwrap().
            let title = format!("{} {action}", key.0.clone().white());
            let p = Paragraph::new(title)
                .style(Style::default().fg(Color::White).reversed())
                .centered();
            f.render_widget(p, first_row[i]);
        }

        let second_row = Layout::default()
            .direction(Direction::Horizontal)
            .constraints::<&Vec<_>>(contraints.as_ref())
            .split(rows[2]);

        for (i, (key, action)) in self.current.iter().take(6).enumerate() {
            // Todo: Remove unwrap().
            let title = format!("{key} {action}");
            let p = Paragraph::new(title)
                .style(Style::default().fg(Color::White).reversed())
                .centered();
            f.render_widget(p, second_row[i]);
        }

        Ok(())
    }
}

#[derive(Debug, Hash, Eq, PartialEq)]
struct KeySymbol(String);

impl fmt::Display for KeySymbol {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Write strictly the first element into the supplied output
        // stream: `f`. Returns `fmt::Result` which indicates whether the
        // operation succeeded or failed. Note that `write!` uses syntax which
        // is very similar to `println!`.
        write!(f, "{}", self.0)
    }
}

impl From<KeyEvent> for KeySymbol {
    fn from(value: KeyEvent) -> Self {
        let code = match value.code {
            KeyCode::Backspace => Self('⌫'.to_string()),
            KeyCode::Enter => Self('⏎'.to_string()),
            KeyCode::Left => Self('←'.to_string()),
            KeyCode::Right => Self('→'.to_string()),
            KeyCode::Up => Self('↑'.to_string()),
            KeyCode::Down => Self('↓'.to_string()),
            KeyCode::Home => Self('🏠'.to_string()),
            KeyCode::End => Self("END".to_string()),
            KeyCode::PageUp => Self("PAGEUP".to_string()),
            KeyCode::PageDown => Self("PAGEDOWN".to_string()),
            KeyCode::Tab => Self('⇥'.to_string()),
            KeyCode::BackTab => Self('⇤'.to_string()),
            KeyCode::Delete => Self('⌦'.to_string()),
            KeyCode::Insert => Self("INS".to_string()),
            KeyCode::F(num) => Self(format!("F-{num}")),
            KeyCode::Char(c) => Self(c.to_string()),
            KeyCode::Null => Self('␀'.to_string()),
            KeyCode::Esc => Self('␛'.to_string()),
            KeyCode::CapsLock => Self('⇪'.to_string()),
            KeyCode::ScrollLock => Self('⇳'.to_string()),
            KeyCode::NumLock => Self('⇭'.to_string()),
            KeyCode::PrintScreen => Self("NA".to_string()),
            KeyCode::Pause => Self("II".to_string()),
            KeyCode::Menu => Self("NA".to_string()),
            KeyCode::KeypadBegin => Self("NA".to_string()),
            KeyCode::Media(_) => Self("NA".to_string()),
            KeyCode::Modifier(_) => Self("NA".to_string()),
        };

        if value.modifiers == KeyModifiers::CONTROL {
            Self(format!("^{}", code))
        } else {
            code
        }
    }
}
