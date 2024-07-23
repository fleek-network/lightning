#![allow(unused)]
use anyhow::Result;
use log::LevelFilter;
use ratatui::prelude::{Color, Constraint, Layout, Modifier, Rect, Style, Widget};
use ratatui::widgets::{Block, Borders, Tabs};
use tokio::sync::mpsc::UnboundedSender;
use tui_logger::{TuiLoggerLevelOutput, TuiLoggerSmartWidget, TuiWidgetEvent, TuiWidgetState};

use super::firewall::form::FirewallFormAction;
use super::Draw;
use crate::app::{ApplicationContext, GlobalAction};
use crate::components::Component;
use crate::config::{ComponentKeyBindings, Config};
use crate::tui::Frame;

pub enum LoggerAction {
    Up,
    Down,
    FilterLeft,
    FilterRight,
    PageUp,
    PageDown,
    Add,
    Remove,
    Hide,
    Focus,
    Toggle,
    Next,
    NavLeft,
    NavRight,
}

impl std::str::FromStr for LoggerAction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "Up" => Ok(LoggerAction::Up),
            "Down" => Ok(LoggerAction::Down),
            "FilterLeft" => Ok(LoggerAction::FilterLeft),
            "FilterRight" => Ok(LoggerAction::FilterRight),
            "PageUp" => Ok(LoggerAction::PageUp),
            "PageDown" => Ok(LoggerAction::PageDown),
            "Add" => Ok(LoggerAction::Add),
            "Remove" => Ok(LoggerAction::Remove),
            "Hide" => Ok(LoggerAction::Hide),
            "Focus" => Ok(LoggerAction::Focus),
            "Toggle" => Ok(LoggerAction::Toggle),
            "Next" => Ok(LoggerAction::Next),
            _ => Err(anyhow::anyhow!("Invalid LoggerAction {s}")),
        }
    }
}

#[derive(Default)]
pub struct Logger {
    states: Vec<TuiWidgetState>,
    tab_names: Vec<&'static str>,
    selected_tab: usize,
    keybindings: ComponentKeyBindings<LoggerAction>,
}

impl Logger {
    pub fn new() -> Self {
        let states = vec![
            TuiWidgetState::new().set_default_display_level(LevelFilter::Info),
            TuiWidgetState::new().set_default_display_level(LevelFilter::Info),
            TuiWidgetState::new().set_default_display_level(LevelFilter::Info),
            TuiWidgetState::new().set_default_display_level(LevelFilter::Info),
            TuiWidgetState::new().set_default_display_level(LevelFilter::Info),
        ];
        let tab_names = vec!["S1", "S2", "S3", "S4", "S5"];
        Self {
            states,
            tab_names,
            selected_tab: 0,
            keybindings: ComponentKeyBindings::default(),
        }
    }

    fn selected_state(&mut self) -> &mut TuiWidgetState {
        &mut self.states[self.selected_tab]
    }

    fn next_tab(&mut self) {
        self.selected_tab = (self.selected_tab + 1) % self.tab_names.len();
    }
}

impl Component for Logger {
    type Context = ApplicationContext;

    fn component_name(&self) -> &'static str {
        "Logger"
    }

    fn register_keybindings(&mut self, config: &Config) {
        self.keybindings = config.keybindings.parse_actions(self.component_name());
    }

    fn handle_event(
        &mut self,
        context: &mut Self::Context,
        event: &[crossterm::event::KeyEvent],
    ) -> Result<Option<crate::app::GlobalAction>> {
        if let Some(action) = self.keybindings.get(event) {
            let state = &mut self.states[self.selected_tab];

            match action {
                LoggerAction::Up => state.transition(TuiWidgetEvent::UpKey),
                LoggerAction::Down => state.transition(TuiWidgetEvent::DownKey),
                LoggerAction::FilterLeft => state.transition(TuiWidgetEvent::LeftKey),
                LoggerAction::FilterRight => state.transition(TuiWidgetEvent::RightKey),
                LoggerAction::PageUp => state.transition(TuiWidgetEvent::PrevPageKey),
                LoggerAction::PageDown => state.transition(TuiWidgetEvent::NextPageKey),
                LoggerAction::Add => state.transition(TuiWidgetEvent::PlusKey),
                LoggerAction::Remove => state.transition(TuiWidgetEvent::MinusKey),
                LoggerAction::Hide => state.transition(TuiWidgetEvent::HideKey),
                LoggerAction::Focus => state.transition(TuiWidgetEvent::FocusKey),
                LoggerAction::Toggle => state.transition(TuiWidgetEvent::SpaceKey),
                LoggerAction::Next => self.next_tab(),
                LoggerAction::NavLeft => context.nav_left(),
                LoggerAction::NavRight => context.nav_right(),                
            }

            return Ok(Some(GlobalAction::Render));
        }

        Ok(None)
    }
}

impl Draw for Logger {
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let chunk: [Rect; 2] =
            Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(area);

        let tabs = Tabs::new(self.tab_names.iter().cloned())
            .block(Block::default().title("States").borders(Borders::ALL))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .select(self.selected_tab);

        f.render_widget(tabs, chunk[0]);

        let logger = TuiLoggerSmartWidget::default()
            .style_error(Style::default().fg(Color::Red))
            .style_debug(Style::default().fg(Color::Green))
            .style_warn(Style::default().fg(Color::Yellow))
            .style_trace(Style::default().fg(Color::Magenta))
            .style_info(Style::default().fg(Color::Cyan))
            .output_separator(':')
            .output_timestamp(Some("%H:%M:%S".to_string()))
            .output_level(Some(TuiLoggerLevelOutput::Abbreviated))
            .output_target(true)
            .output_file(true)
            .output_line(true)
            .title_log("Logs")
            .title_target("Targets")
            .state(self.selected_state());

        f.render_widget(logger, chunk[1]);

        Ok(())
    }
}
