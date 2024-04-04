use color_eyre::eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::prelude::{Constraint, Direction, Layout, Rect};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::action::Action;
use crate::components::fps::FpsCounter;
use crate::components::home::Home;
use crate::components::side_bar::SideBar;
use crate::components::Component;
use crate::config::Config;
use crate::mode::Mode;
use crate::tui;
use crate::tui::Frame;

pub struct App {
    pub config: Config,
    pub tick_rate: f64,
    pub frame_rate: f64,
    pub home: Home,
    pub side_bar: SideBar,
    pub should_quit: bool,
    pub should_suspend: bool,
    pub mode: Mode,
    pub last_tick_key_events: Vec<KeyEvent>,
}

impl App {
    pub fn new(tick_rate: f64, frame_rate: f64) -> Result<Self> {
        let _home = Home::new();
        let _fps = FpsCounter::default();
        let side_bar = SideBar::new();
        let config = Config::new()?;
        let mode = Mode::Home;
        Ok(Self {
            tick_rate,
            frame_rate,
            home: _home,
            side_bar,
            should_quit: false,
            should_suspend: false,
            config,
            mode,
            last_tick_key_events: Vec::new(),
        })
    }

    pub fn draw_components(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(15),
                Constraint::Percentage((100_u16).saturating_sub(15)),
            ])
            .split(f.size());

        self.side_bar.draw(f, main_chunks[0])?;
        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        let (action_tx, mut action_rx) = mpsc::unbounded_channel();

        let mut tui = tui::Tui::new()?
            .tick_rate(self.tick_rate)
            .frame_rate(self.frame_rate);
        // tui.mouse(true);
        tui.enter()?;

        self.home.register_action_handler(action_tx.clone())?;
        self.home.register_config_handler(self.config.clone())?;
        self.home.init(tui.size()?)?;

        self.side_bar.register_action_handler(action_tx.clone())?;
        self.side_bar.register_config_handler(self.config.clone())?;
        self.side_bar.init(tui.size()?)?;

        loop {
            if let Some(e) = tui.next().await {
                match e {
                    tui::Event::Quit => action_tx.send(Action::Quit)?,
                    tui::Event::Tick => action_tx.send(Action::Tick)?,
                    tui::Event::Render => action_tx.send(Action::Render)?,
                    tui::Event::Resize(x, y) => action_tx.send(Action::Resize(x, y))?,
                    tui::Event::Key(key) => {
                        if let Some(keymap) = self.config.keybindings.get(&self.mode) {
                            if let Some(action) = keymap.get(&vec![key]) {
                                log::info!("Got action: {action:?}");
                                action_tx.send(action.clone())?;
                            } else {
                                // If the key was not handled as a single key action,
                                // then consider it for multi-key combinations.
                                self.last_tick_key_events.push(key);

                                // Check for multi-key combinations
                                if let Some(action) = keymap.get(&self.last_tick_key_events) {
                                    log::info!("Got action: {action:?}");
                                    action_tx.send(action.clone())?;
                                }
                            }
                        };
                    },
                    _ => {},
                }

                // Todo: Handle events better here for components.
                if let Some(action) = self.side_bar.handle_events(Some(e.clone()))? {
                    action_tx.send(action)?;
                }
            }

            while let Ok(action) = action_rx.try_recv() {
                if action != Action::Tick && action != Action::Render {
                    log::debug!("{action:?}");
                }
                match action {
                    Action::Tick => {
                        self.last_tick_key_events.drain(..);
                    },
                    Action::Quit => self.should_quit = true,
                    Action::Suspend => self.should_suspend = true,
                    Action::Resume => self.should_suspend = false,
                    Action::Resize(w, h) => {
                        tui.resize(Rect::new(0, 0, w, h))?;
                        tui.draw(|f| {
                            if let Err(e) = self.draw_components(f, f.size()) {
                                action_tx
                                    .send(Action::Error(format!("Failed to draw: {:?}", e)))
                                    .unwrap();
                            }
                        })?;
                    },
                    Action::Render => {
                        tui.draw(|f| {
                            if let Err(e) = self.draw_components(f, f.size()) {
                                action_tx
                                    .send(Action::Error(format!("Failed to draw: {:?}", e)))
                                    .unwrap();
                            }
                        })?;
                    },
                    _ => {},
                }

                // Todo: Handle events better here for components.
                if let Some(action) = self.side_bar.update(action.clone())? {
                    action_tx.send(action)?;
                }
            }

            if self.should_suspend {
                tui.suspend()?;
                action_tx.send(Action::Resume)?;
                tui = tui::Tui::new()?
                    .tick_rate(self.tick_rate)
                    .frame_rate(self.frame_rate);
                // tui.mouse(true);
                tui.enter()?;
            } else if self.should_quit {
                tui.stop()?;
                break;
            }
        }
        tui.exit()?;
        Ok(())
    }
}
