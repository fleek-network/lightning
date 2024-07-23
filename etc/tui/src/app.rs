use std::collections::HashMap;

use anyhow::Result;
use crossterm::event::KeyEvent;
use lightning_guard::ConfigSource;
use log::debug;
use ratatui::prelude::{Constraint, Direction, Layout, Rect};

use crate::components::firewall::FireWall;
use crate::components::home::Home;
#[cfg(feature = "logger")]
use crate::components::logger::Logger;
use crate::components::navigator::{NavDirection, Navigator};
use crate::components::profile::Profile;
use crate::components::prompt::{Prompt, PromptChange};
use crate::components::summary::Summary;
use crate::components::{Component, Draw};
use crate::config::Config;
use crate::tui;
use crate::tui::Frame;

/// A special case of behaviour that the main app can take.
///
/// These are typically explicity handled branches of the main loop.
pub enum GlobalAction {
    Render,
    Quit,
}

pub struct ApplicationContext {
    /// The active component. Set by the navigator
    active_component: &'static str,
    /// Set by user components to signal a navigation event.
    nav: Option<NavDirection>,
    /// The prompt change if any. This is useful for subcomponents that need to change the prompt.
    pub prompt_change: Option<PromptChange>,
}

impl ApplicationContext {
    pub fn nav_left(&mut self) {
        self.nav = Some(NavDirection::Left);
        self.prompt_change = Some(PromptChange::ActiveComponent);
    }

    pub fn nav_right(&mut self) {
        self.nav = Some(NavDirection::Right);
        self.prompt_change = Some(PromptChange::ActiveComponent);
    }

    #[inline]
    pub fn component_changed(&self) -> bool {
        self.nav.is_some()
    }

    pub fn change_prompt(&mut self, prompt: PromptChange) {
        self.prompt_change = Some(prompt);
    }

    pub fn active_component(&self) -> &'static str {
        self.active_component
    }
}

impl ApplicationContext {
    // prompt should be set between top level component changes
    pub unsafe fn nav(&mut self) -> &mut Option<NavDirection> {
        &mut self.nav
    }
}

pub struct App {
    config: Config,
    navigator: Navigator,
    prompt: Prompt,
    summary: Summary,
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
        let mut context = ApplicationContext {
            active_component: "Home",
            nav: None,
            prompt_change: Some(PromptChange::ActiveComponent),
        };
        prompt.update_state(&mut context);

        Ok(Self {
            prompt,
            navigator: Navigator::new(),
            config: config.clone(),
            tick_rate,
            frame_rate,
            last_tick_key_events: Vec::new(),
            context,
            summary: Summary::new(),
            components: HashMap::new(),
            config_source: src,
        })
    }

    fn active_component_ref(&mut self) -> &mut Box<dyn Component<Context = ApplicationContext>> {
        self.components
            .get_mut(self.navigator.active_component())
            .expect("A valid active component name from navigator")
    }

    fn with_component<C>(&mut self, mut component: C) -> &mut Self
    where
        C: Component<Context = ApplicationContext>,
        C: 'static,
    {
        let _ = component.register_keybindings(&self.config);

        self.navigator.push_tab(component.component_name());

        self.components
            .insert(component.component_name(), Box::new(component));

        self
    }

    fn draw_components(&mut self, f: &mut Frame<'_>, _area: Rect) -> Result<()> {
        let body_footer_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Length(3)])
            .split(f.size());

        let content_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage((100_u16).saturating_sub(25)),
            ])
            .split(body_footer_area[0]);

        self.summary.draw( f, content_area[0])?;

        let navigation_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(100)])
            .split(content_area[1]);

        // note: the order here is load bearing
        // the prompt requires that the navigator determines the active component first
        // so that we can update the context with it
        // the tldr is that prompt may be displaying keybindins for subcomponents, so this is mostly
        // a no op except for top level component changes, in which case the order matters
        // these are special cases of components that dont have thier own keybindings
        {
            self.navigator.update_state(&mut self.context);

            self.navigator
                .draw(f, navigation_area[0])?;

            self.context.active_component = self.navigator.active_component();

            self.prompt.update_state(&mut self.context);

            self.prompt
                .draw(f, body_footer_area[1])?;
        }

        let content = Layout::default()
            .vertical_margin(3)
            .horizontal_margin(3)
            .constraints([Constraint::Percentage(100)])
            .split(navigation_area[0]);

        let component = self
            .components
            .get_mut(self.navigator.active_component())
            .expect("A valid component from the navigator");

        component.draw(f, content[0])?;

        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        debug!(target: "Main Tui App", "Running!");

        let config_source = self.config_source.clone();
        let mut this = self
            .with_component(Home::new())
            .with_component(Profile::new(config_source.clone()).await?)
            .with_component(FireWall::new(config_source.clone()));

        #[cfg(feature = "logger")]
        {
            this = this.with_component(Logger::new());
        }

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
                        let _ = this.active_component_ref().tick();

                        None
                    },
                    tui::Event::Quit => {
                        log::warn!("got tui event quit");
                        break;
                        // todo!
                    },
                    tui::Event::Key(key) => {
                        let component = this.navigator.active_component();
                        let component = this
                            .components
                            .get_mut(component)
                            .expect("A valid component given by the navigator.");

                        let first_action = match component.handle_event(&mut this.context, &[key]) {
                            Ok(action) => action,
                            Err(e) => {
                                log::error!("Failed to handle event: {:?}", e);
                                None
                            },
                        };

                        this.last_tick_key_events.push(key);
                        let second_action = if this.last_tick_key_events.len() > 1 {
                            match component
                                .handle_event(&mut this.context, &this.last_tick_key_events)
                            {
                                Ok(action) => action,
                                Err(e) => {
                                    log::error!("Failed to handle event: {:?}", e);
                                    None
                                },
                            }
                        } else {
                            None
                        };

                        if second_action.is_some() && first_action.is_some() {
                            log::warn!(
                                "Got two competing actions, you have key bindings that share a prefix"
                            );
                        }

                        second_action.or(first_action)
                    },
                    // These calls come from a timer and a resize therefore there c
                    // an be no component change as sa side effect.
                    tui::Event::Resize(w, h) => {
                        tui.resize(Rect::new(0, 0, w, h))?;
                        tui.draw(|f| {
                            if let Err(e) = this.draw_components(f, f.size()) {
                                log::error!("Failed to draw: {:?}", e);
                            }
                        })?;

                        None
                    },
                    // See above
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

                            this.context.active_component = this.navigator.active_component();
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
