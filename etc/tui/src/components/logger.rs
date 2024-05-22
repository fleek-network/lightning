#![allow(unused)]
use anyhow::Result;
use log::LevelFilter;
use ratatui::prelude::{Color, Constraint, Layout, Modifier, Rect, Style, Widget};
use ratatui::widgets::{Block, Borders, Tabs};
use tokio::sync::mpsc::UnboundedSender;
use tui_logger::{TuiLoggerLevelOutput, TuiLoggerSmartWidget, TuiWidgetEvent, TuiWidgetState};

use crate::action::Action;
use crate::components::Component;
use crate::config::Config;
use crate::tui::Frame;

#[derive(Default)]
pub struct Logger {
    states: Vec<TuiWidgetState>,
    tab_names: Vec<&'static str>,
    selected_tab: usize,
    config: Config,
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
            config: Config::default(),
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
    fn register_action_handler(&mut self, _tx: UnboundedSender<Action>) -> Result<()> {
        Ok(())
    }

    fn register_config_handler(&mut self, _config: Config) -> Result<()> {
        Ok(())
    }

    fn update(&mut self, action: Action) -> anyhow::Result<Option<Action>> {
        let state = self.selected_state();
        match action {
            Action::Up => state.transition(TuiWidgetEvent::UpKey),
            Action::Down => state.transition(TuiWidgetEvent::DownKey),
            Action::FilterLeft => state.transition(TuiWidgetEvent::LeftKey),
            Action::FilterRight => state.transition(TuiWidgetEvent::RightKey),
            Action::PageUp => state.transition(TuiWidgetEvent::PrevPageKey),
            Action::PageDown => state.transition(TuiWidgetEvent::NextPageKey),
            Action::Add => state.transition(TuiWidgetEvent::PlusKey),
            Action::Remove => state.transition(TuiWidgetEvent::MinusKey),
            Action::Hide => state.transition(TuiWidgetEvent::HideKey),
            Action::Focus => state.transition(TuiWidgetEvent::FocusKey),
            Action::Toggle => state.transition(TuiWidgetEvent::SpaceKey),
            Action::Next => self.next_tab(),
            _ => return Ok(None),
        }

        Ok(Some(Action::Render))
    }

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
