use anyhow::Result;
use ratatui::prelude::{Rect, Style};
use ratatui::style::Stylize;
use ratatui::widgets::{Block, Borders, Tabs};
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, Draw, Frame};
use crate::app::{ApplicationContext, GlobalAction};
use crate::config::{ComponentKeyBindings, Config};

pub enum NavDirection {
    Left,
    Right,
}

/// Component for switching between tabs.
pub struct Navigator {
    selected_tab: usize,
    tabs: Vec<&'static str>,
}

impl Navigator {
    pub fn new() -> Self {
        Self {
            selected_tab: 0,
            tabs: vec![],
        }
    }

    pub fn push_tab(&mut self, tab: &'static str) {
        self.tabs.push(tab);
    }

    pub fn nav_left(&mut self) {
        if self.selected_tab > 0 {
            self.selected_tab -= 1;
        }
    }

    pub fn nav_right(&mut self) {
        if self.selected_tab < self.tabs.len() - 1 {
            self.selected_tab += 1;
        }
    }

    #[inline]
    pub fn active_component(&self) -> &'static str {
        self.tabs[self.selected_tab]
    }

    pub fn update_state(&mut self, context: &mut ApplicationContext) {
        unsafe {
            if let Some(nav) = context.nav().take() {
                match nav {
                    NavDirection::Left => self.nav_left(),
                    NavDirection::Right => self.nav_right(),
                }
            }
        }
    }
}

impl Draw for Navigator {
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