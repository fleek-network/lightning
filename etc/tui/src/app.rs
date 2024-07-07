use std::collections::HashMap;
use std::marker::PhantomData;
use std::str::FromStr;

use anyhow::Result;
use crossterm::event::KeyEvent;
use lightning_guard::ConfigSource;
use log::debug;
use ratatui::prelude::{Constraint, Direction, Layout, Rect};
#[cfg(feature = "logger")]
use socket_logger::Listener;
#[cfg(feature = "logger")]
use tokio::net::UnixListener;
use tokio::sync::mpsc;

// use crate::components::firewall::form::FirewallForm;
// use crate::components::firewall::FireWall;
use crate::components::home::Home;
// #[cfg(feature = "logger")]
// use crate::components::logger::Logger;
use crate::components::navigator::Navigator;
use crate::components::profile::Profile;
use crate::components::prompt::Prompt;
use crate::components::summary::Summary;
use crate::components::{self, Component, Draw};
use crate::config::Config;
use crate::tui;
use crate::tui::Frame;
#[cfg(feature = "logger")]
use crate::utils::SOCKET_LOGGER_FOLDER;

/// A special case of behaviour that the main app can take.
///
/// These are typically explicity handled branches of the main loop.
pub enum GlobalAction {
    Render,
    Quit,
}

pub struct ApplicationContext {
    navigator: Navigator,
    prompt: Prompt,
    summary: Summary,
}

/// Actions that can by the main app.
#[derive(Debug, Clone, Copy)]
pub enum AppAction {
    Suspend,
    NavLeft,
    NavRight,
    Quit,
}

impl FromStr for AppAction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "NavLeft" => Ok(Self::NavLeft),
            "NavRight" => Ok(Self::NavRight),
            "Quit" => Ok(Self::Quit),
            "Suspend" => Ok(Self::Suspend),
            _ => Err(anyhow::anyhow!("Invalid AppAction {s}")),
        }
    }
}

impl ApplicationContext {
    pub fn handle_app_action(&mut self, action: AppAction) -> Option<GlobalAction> {
        match action {
            AppAction::NavLeft => {
                self.nav_left();
                Some(GlobalAction::Render)
            },
            AppAction::NavRight => {
                self.nav_right();
                Some(GlobalAction::Render)
            },
            AppAction::Quit => Some(GlobalAction::Quit),
            // todo
            AppAction::Suspend => None,
        }
    }

    pub fn nav_left(&mut self) {
        self.navigator.nav_left();
        self.prompt.update_state(self.navigator.active_component());
    }

    pub fn nav_right(&mut self) {
        self.navigator.nav_right();
        self.prompt.update_state(self.navigator.active_component());
    }

    pub fn change_prompt(&mut self, prompt: &'static str) {
        self.prompt.update_state(prompt);
    }

    pub fn active_component(&self) -> &'static str {
        self.navigator.active_component()
    }
}

pub struct App {
    pub config: Config,
    pub tick_rate: f64,
    pub frame_rate: f64,
    pub last_tick_key_events: Vec<KeyEvent>,
    pub context: ApplicationContext,
    pub components: HashMap<&'static str, Box<dyn Component<Context = ApplicationContext>>>,
    pub config_source: ConfigSource,
}

impl App {
    pub fn new(tick_rate: f64, frame_rate: f64, src: ConfigSource) -> Result<Self> {
        let config = Config::new()?;
        let mut prompt = Prompt::new(config.clone());
        prompt.update_state("Home");

        Ok(Self {
            config: config.clone(),
            tick_rate,
            frame_rate,
            last_tick_key_events: Vec::new(),
            context: ApplicationContext {
                navigator: Navigator::new(),
                prompt: prompt,
                summary: Summary::new(),
            },
            components: HashMap::new(),
            config_source: src,
        })
    }

    fn active_component(&mut self) -> &mut Box<dyn Component<Context = ApplicationContext>> {
        self.components
            .get_mut(self.context.navigator.active_component())
            .expect("A valid active component name from navigator")
    }

    fn with_component<C>(&mut self, mut component: C) -> &mut Self
    where
        C: Component<Context = ApplicationContext>,
        C: 'static,
    {
        let _ = component.register_keybindings(&self.config);

        self.context.navigator.push_tab(component.component_name());
        self.components
            .insert(component.component_name(), Box::new(component));

        self
    }

    fn draw_components(&mut self, f: &mut Frame<'_>, _area: Rect) -> Result<()> {
        let body_footer_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Length(3)])
            .split(f.size());

        self.context.prompt.draw(&mut (), f, body_footer_area[1])?;

        let content_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage((100_u16).saturating_sub(25)),
            ])
            .split(body_footer_area[0]);

        self.context.summary.draw(&mut (), f, content_area[0])?;

        let navigation_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(100)])
            .split(content_area[1]);

        self.context
            .navigator
            .draw(&mut (), f, navigation_area[0])?;

        let content = Layout::default()
            .vertical_margin(3)
            .horizontal_margin(3)
            .constraints([Constraint::Percentage(100)])
            .split(navigation_area[0]);

        let component = self
            .components
            .get_mut(self.context.navigator.active_component())
            .expect("A valid component from the navigator");
        
        component.draw(&mut self.context, f, content[0])?;

        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        debug!(target: "Main Tui App", "Running!");

        let config_source = self.config_source.clone();
        let this = self
            .with_component(components::home::Home::new())
            // .with_component(components::firewall::FireWall::new(config_source.clone()))
            .with_component(components::profile::Profile::new(config_source.clone()).await?);

        let mut tui = tui::Tui::new()?
            .tick_rate(this.tick_rate)
            .frame_rate(this.frame_rate);

        tui.enter()?;

        // draw components for the first time
        tui.draw(|f| {
            if let Err(e) = this.draw_components(f, f.size()) {
                log::error!("Failed to draw: {:?}", e);
            }
        })?;

        loop {
            if let Some(event) = tui.next().await {
                let maybe_global_action = match event {
                    tui::Event::Tick => {
                        this.last_tick_key_events.drain(..);
                        let _ = this.active_component().tick();

                        None
                    },
                    tui::Event::Quit => {
                        log::warn!("got tui event quit");
                        break;
                        // todo!
                    },
                    tui::Event::Key(key) => {
                        let component = this.context.navigator.active_component();
                        let component = this
                            .components
                            .get_mut(component)
                            .expect("A valid component given by the navigator.");

                        if component.is_known_event(&[key]) {
                            match component.handle_known_event(&mut this.context, &[key]) {
                                Ok(action) => action,
                                Err(e) => {
                                    log::error!("Failed to handle event: {:?}", e);
                                    None
                                },
                            }
                        } else {
                            this.last_tick_key_events.push(key);

                            if component.is_known_event(&this.last_tick_key_events) {
                                match component.handle_known_event(
                                    &mut this.context,
                                    &this.last_tick_key_events,
                                ) {
                                    Ok(action) => action,
                                    Err(e) => {
                                        log::error!("Failed to handle event: {:?}", e);
                                        None
                                    },
                                }
                            } else {
                                None
                            }
                        }
                    },
                    tui::Event::Resize(w, h) => {
                        tui.resize(Rect::new(0, 0, w, h))?;
                        tui.draw(|f| {
                            if let Err(e) = this.draw_components(f, f.size()) {
                                log::error!("Failed to draw: {:?}", e);
                            }
                        })?;

                        None
                    },
                    tui::Event::Render => {
                        tui.draw(|f| {
                            if let Err(e) = this.draw_components(f, f.size()) {
                                log::error!("Failed to draw: {:?}", e);
                            }
                        })?;

                        None
                    },
                    tui::Event::Mouse(_) => None,
                    tui::Event::Paste(_) => None,
                    tui::Event::Init => None,
                    tui::Event::Error => None,
                };

                if let Some(global_action) = maybe_global_action {
                    match global_action {
                        GlobalAction::Render => {
                            tui.draw(|f| {
                                if let Err(e) = this.draw_components(f, f.size()) {
                                    log::error!("Failed to draw: {:?}", e);
                                }
                            })?;
                        },
                        GlobalAction::Quit => {
                            log::warn!("Quitting!");
                            break;
                        },
                    }
                }
            }
        }

        Ok(())
    }
}
