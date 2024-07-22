use std::fmt;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::{Alignment, Color, Constraint, Direction, Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::widgets::Paragraph;
use tokio::sync::mpsc::UnboundedSender;
use super::{Component, Draw, Frame};
use crate::config::Config;

const MAX_KEYS_PER_ROW: usize = 8;

/// Component for displaying key bindings and error messages.
#[derive(Default)]
pub struct Prompt {
    current: Vec<(KeySymbol, String)>,
    message: Option<String>,
    config: Config,
}

impl Prompt {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_message(&mut self, message: String) {
        self.message = Some(message);
    }

    pub fn update_state(&mut self, mode: &'static str) {
        if let Some(keys) = self.config.keybindings.get(&mode) {
            let keys = keys.clone();
            let codes = keys
                .into_iter()
                .map(|(key, value)| {
                    debug_assert!(key.len() == 1);
                    (KeySymbol::from(key[0]), value)
                })
                .collect::<Vec<_>>();

            self.current = codes;

            // Remove an old message.
            self.message.take();
        }
    }
}

impl Draw for Prompt {
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area);

        if let Some(e) = self.message.as_ref() {
            f.render_widget(
                Paragraph::new(e.as_str())
                    .alignment(Alignment::Left)
                    .style(Style::default().fg(Color::Red).reversed()),
                rows[0],
            );
        }

        let contraints = [Constraint::Percentage(100); MAX_KEYS_PER_ROW];

        let first_row = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(contraints[..MAX_KEYS_PER_ROW].as_ref())
            .split(rows[1]);

        for i in 0..contraints.len() {
            let paragraph = if let Some((symbol, action)) = self.current.get(i) {
                let title = format!("[{symbol}] {action}");
                Paragraph::new(title)
            } else {
                Paragraph::default()
            };

            let chunk = first_row[i];

            f.render_widget(
                paragraph
                    .style(Style::default().fg(Color::White).reversed())
                    .centered(),
                chunk,
            );
        }

        Ok(())
    }
}
#[derive(Debug, Hash, Eq, PartialEq)]
struct KeySymbol(String);

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
            KeyCode::Esc => Self("ESC".to_string()),
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
        } else if value.modifiers == KeyModifiers::SHIFT {
            Self(format!("⇧{}", code))
        } else {
            code
        }
    }
}

impl fmt::Display for KeySymbol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
