use color_eyre::eyre::Result;
use crossterm::event::KeyEvent;
use ebpf_service::map::storage::Storage;
use ratatui::layout::Flex;
use ratatui::prelude::{Constraint, Direction, Layout, Rect};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::action::Action;
use crate::components::firewall::FireWall;
use crate::components::home::Home;
use crate::components::navigator::Navigator;
use crate::components::prompt::Prompt;
use crate::components::summary::Summary;
use crate::components::Component;
use crate::config::Config;
use crate::mode::Mode;
use crate::tui;
use crate::tui::Frame;

pub struct App {
    pub config: Config,
    pub tick_rate: f64,
    pub frame_rate: f64,
    pub should_quit: bool,
    pub should_suspend: bool,
    pub mode: Mode,
    pub last_tick_key_events: Vec<KeyEvent>,
    // Components.
    pub home: Home,
    pub summary: Summary,
    pub prompt: Prompt,
    pub navigator: Navigator,
    pub firewall: FireWall,
}

impl App {
    pub fn new(tick_rate: f64, frame_rate: f64, storage: Storage) -> Result<Self> {
        let mode = Mode::Home;
        let home = Home::new();
        let firewall = FireWall::new(storage);
        let summary = Summary::new();
        let prompt = Prompt::new();
        let navigator = Navigator::new();
        let config = Config::new()?;
        Ok(Self {
            tick_rate,
            frame_rate,
            home,
            summary,
            prompt,
            navigator,
            firewall,
            should_quit: false,
            should_suspend: false,
            config,
            mode,
            last_tick_key_events: Vec::new(),
        })
    }

    fn update_components(&mut self, action: Action) -> Result<Option<Action>> {
        let maybe_action = match self.mode {
            Mode::Home => self.home.update(action.clone())?,
            Mode::Firewall => self.firewall.update(action.clone())?,
            Mode::FirewallNewEntry => self.firewall.update(action.clone())?,
            _ => None,
        };

        if maybe_action.is_none() {
            self.navigator.update(action)
        } else {
            Ok(maybe_action)
        }
    }

    fn handle_event(&mut self, event: tui::Event) -> Result<Option<Action>> {
        match self.mode {
            Mode::Home => self.home.handle_events(Some(event)),
            Mode::Firewall => self.firewall.handle_events(Some(event)),
            Mode::FirewallNewEntry => self.firewall.handle_events(Some(event)),
            _ => {
                // We ignore navigation here.
                Ok(None)
            },
        }
    }

    fn draw_components(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let body_footer_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100_u16).saturating_sub(10)),
                Constraint::Percentage(10),
            ])
            .split(f.size());

        self.prompt.draw(f, body_footer_area[1])?;

        let content_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(35),
                Constraint::Percentage((100_u16).saturating_sub(35)),
            ])
            .split(body_footer_area[0]);

        self.summary.draw(f, content_area[0])?;

        let navigation_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(100)])
            .split(content_area[1]);

        self.navigator.draw(f, navigation_area[0])?;

        let content = Layout::default()
            .vertical_margin(3)
            .horizontal_margin(3)
            .constraints([Constraint::Percentage(100)])
            .split(navigation_area[0]);

        match self.mode {
            Mode::Firewall | Mode::FirewallNewEntry => {
                self.firewall.draw(f, content[0])?;
            },
            _ => {},
        }

        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        let (action_tx, mut action_rx) = mpsc::unbounded_channel();

        let mut tui = tui::Tui::new()?
            .tick_rate(self.tick_rate)
            .frame_rate(self.frame_rate);
        tui.enter()?;

        self.home.register_action_handler(action_tx.clone())?;
        self.home.register_config_handler(self.config.clone())?;
        self.home.init(tui.size()?)?;

        self.summary.register_action_handler(action_tx.clone())?;
        self.summary.register_config_handler(self.config.clone())?;
        self.summary.init(tui.size()?)?;

        self.prompt.register_action_handler(action_tx.clone())?;
        self.prompt.register_config_handler(self.config.clone())?;
        self.prompt.init(tui.size()?)?;
        // On start up, state has not been set yet so we do that now.
        self.prompt.update_state(self.mode);

        self.firewall.register_action_handler(action_tx.clone())?;
        self.firewall.register_config_handler(self.config.clone())?;
        self.firewall.init(tui.size()?)?;

        self.navigator.register_action_handler(action_tx.clone())?;
        self.navigator
            .register_config_handler(self.config.clone())?;
        self.navigator.init(tui.size()?)?;

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

                if let Some(action) = self.handle_event(e)? {
                    action_tx.send(action)?;
                }
            }

            while let Ok(action) = action_rx.try_recv() {
                if action != Action::Tick && action != Action::Render {
                    log::debug!("{action:?}");
                }
                match &action {
                    Action::Tick => {
                        self.last_tick_key_events.drain(..);
                    },
                    Action::Quit => self.should_quit = true,
                    Action::Suspend => self.should_suspend = true,
                    Action::Resume => self.should_suspend = false,
                    Action::Resize(w, h) => {
                        tui.resize(Rect::new(0, 0, *w, *h))?;
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
                    Action::UpdateMode(mode) => {
                        self.mode = *mode;
                        self.prompt.update_state(self.mode);
                        tui.draw(|f| {
                            if let Err(e) = self.draw_components(f, f.size()) {
                                action_tx
                                    .send(Action::Error(format!("Failed to draw: {:?}", e)))
                                    .unwrap();
                            }
                        })?;
                    },
                    Action::Error(e) => {
                        self.prompt.new_message(e.clone());
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

                if let Some(action) = self.update_components(action)? {
                    action_tx.send(action)?;
                }
            }

            if self.should_suspend {
                tui.suspend()?;
                action_tx.send(Action::Resume)?;
                tui = tui::Tui::new()?
                    .tick_rate(self.tick_rate)
                    .frame_rate(self.frame_rate);
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
