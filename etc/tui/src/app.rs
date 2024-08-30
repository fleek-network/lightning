use anyhow::Result;
use crossterm::event::KeyEvent;
use lightning_guard::map::{FileRule, PacketFilterRule};
use lightning_guard::{map, ConfigSource};
use log::debug;
use ratatui::prelude::{Constraint, Direction, Layout, Rect};
#[cfg(feature = "logger")]
use socket_logger::Listener;
#[cfg(feature = "logger")]
use tokio::net::UnixListener;
use tokio::sync::mpsc;

use crate::action::Action;
use crate::components::firewall::{FireWall, FirewallForm};
use crate::components::home::Home;
#[cfg(feature = "logger")]
use crate::components::logger::Logger;
use crate::components::navigator::Navigator;
use crate::components::profile::{Profile, ProfileForm, ProfileView, RuleForm};
use crate::components::prompt::Prompt;
use crate::components::summary::Summary;
use crate::components::Component;
use crate::config::Config;
use crate::mode::Mode;
use crate::state::State;
use crate::tui;
use crate::tui::Frame;
#[cfg(feature = "logger")]
use crate::utils::SOCKET_LOGGER_FOLDER;

pub struct App {
    pub config: Config,
    pub tick_rate: f64,
    pub frame_rate: f64,
    pub should_quit: bool,
    pub should_suspend: bool,
    pub state: State,
    pub mode: Mode,
    pub last_tick_key_events: Vec<KeyEvent>,
    // Components.
    pub home: Home,
    pub summary: Summary,
    pub prompt: Prompt,
    pub navigator: Navigator,
    pub firewall: FireWall,
    pub profiles: Profile,
    pub profile_view: ProfileView,
    pub profile_form: ProfileForm,
    pub profile_rule_form: RuleForm,
    pub filter_form: FirewallForm,
    #[cfg(feature = "logger")]
    pub logger: Logger,
}

impl App {
    pub fn new(tick_rate: f64, frame_rate: f64, src: ConfigSource) -> Result<Self> {
        let mode = Mode::Home;
        let home = Home::new();
        let firewall = FireWall::new(src.clone());
        #[cfg(feature = "logger")]
        let logger = Logger::new();
        let summary = Summary::new();
        let prompt = Prompt::new();
        let navigator = Navigator::new();
        let profiles = Profile::new(src.clone());
        let config = Config::new()?;
        Ok(Self {
            tick_rate,
            frame_rate,
            home,
            #[cfg(feature = "logger")]
            logger,
            summary,
            prompt,
            navigator,
            firewall,
            profiles,
            profile_view: ProfileView::new(src.clone()),
            profile_form: ProfileForm::new(),
            profile_rule_form: RuleForm::new(),
            should_quit: false,
            should_suspend: false,
            config,
            mode,
            last_tick_key_events: Vec::new(),
            state: State::new(src),
            filter_form: FirewallForm::new(),
        })
    }

    async fn load_state(&mut self) -> Result<()> {
        self.state.load_profiles().await?;
        self.state.load_packet_filter_rules().await?;
        Ok(())
    }

    fn update_components(&mut self, action: Action) -> Result<Option<Action>> {
        let ctx = &mut self.state;
        let maybe_action = match self.mode {
            Mode::Home => self.home.update(action.clone(), ctx)?,
            Mode::Firewall => self.firewall.update(action.clone(), ctx)?,
            Mode::FirewallEdit => self.firewall.update(action.clone(), ctx)?,
            Mode::FirewallForm => self.filter_form.update(action.clone(), ctx)?,
            Mode::Profiles => self.profiles.update(action.clone(), ctx)?,
            Mode::ProfilesEdit => self.profiles.update(action.clone(), ctx)?,
            Mode::ProfileView => self.profile_view.update(action.clone(), ctx)?,
            Mode::ProfileViewEdit => self.profile_view.update(action.clone(), ctx)?,
            Mode::ProfileForm => self.profile_form.update(action.clone(), ctx)?,
            Mode::ProfileRuleForm => self.profile_rule_form.update(action.clone(), ctx)?,
            #[cfg(feature = "logger")]
            Mode::Logger => self.logger.update(action.clone(), ctx)?,
            #[cfg(not(feature = "logger"))]
            Mode::Logger => None,
        };

        if maybe_action.is_none() {
            self.navigator.update(action, ctx)
        } else {
            Ok(maybe_action)
        }
    }

    fn handle_event(&mut self, event: tui::Event) -> Result<Option<Action>> {
        let ctx = &mut self.state;
        match self.mode {
            Mode::Home => self.home.handle_events(Some(event), ctx),
            Mode::Firewall => self.firewall.handle_events(Some(event), ctx),
            Mode::FirewallEdit => self.firewall.handle_events(Some(event), ctx),
            Mode::FirewallForm => self.filter_form.handle_events(Some(event), ctx),
            Mode::Profiles => self.profiles.handle_events(Some(event), ctx),
            Mode::ProfilesEdit => self.profiles.handle_events(Some(event), ctx),
            Mode::ProfileView => self.profile_view.handle_events(Some(event), ctx),
            Mode::ProfileViewEdit => self.profile_view.handle_events(Some(event), ctx),
            Mode::ProfileForm => self.profile_form.handle_events(Some(event), ctx),
            Mode::ProfileRuleForm => self.profile_rule_form.handle_events(Some(event), ctx),
            #[cfg(feature = "logger")]
            Mode::Logger => self.logger.handle_events(Some(event), ctx),
            #[cfg(not(feature = "logger"))]
            Mode::Logger => Ok(None),
        }
    }

    fn draw_components(&mut self, f: &mut Frame<'_>, _area: Rect) -> Result<()> {
        let body_footer_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Length(3)])
            .split(f.size());

        self.prompt.draw(f, body_footer_area[1])?;

        let content_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage((100_u16).saturating_sub(25)),
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
            Mode::Home => {
                self.home.draw(f, content[0])?;
            },
            Mode::Firewall | Mode::FirewallEdit => {
                self.firewall.draw(f, content[0])?;
            },
            Mode::FirewallForm => {
                log::trace!("Drawing filter form");
                self.filter_form.draw(f, content[0])?;
            },
            Mode::Profiles | Mode::ProfilesEdit => {
                self.profiles.draw(f, content[0])?;
            },
            Mode::ProfileView | Mode::ProfileViewEdit => {
                self.profile_view.draw(f, content[0])?;
            },
            Mode::ProfileForm => {
                self.profile_form.draw(f, content[0])?;
            },
            Mode::ProfileRuleForm => {
                self.profile_rule_form.draw(f, content[0])?;
            },
            #[cfg(feature = "logger")]
            Mode::Logger => {
                self.logger.draw(f, content[0])?;
            },
            #[cfg(not(feature = "logger"))]
            Mode::Logger => {
                self.home.draw(f, content[0])?;
            },
        }

        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        debug!(target: "Main Tui App", "Running!");

        let (action_tx, mut action_rx) = mpsc::unbounded_channel();

        #[cfg(feature = "logger")]
        {
            if !SOCKET_LOGGER_FOLDER.as_path().try_exists()? {
                tokio::fs::create_dir_all(SOCKET_LOGGER_FOLDER.as_path()).await?;
            }

            let mut bind_path = SOCKET_LOGGER_FOLDER.to_path_buf();
            bind_path.push("ctrl");
            let _ = tokio::fs::remove_file(bind_path.as_path()).await;
            let listener = UnixListener::bind(bind_path)?;

            let action_tx_clone = action_tx.clone();
            tokio::spawn(async move {
                if let Err(e) = Listener::new(listener).run().await {
                    let _ = action_tx_clone.send(Action::Error(e.to_string()));
                }
            });
        }

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

        self.navigator.register_action_handler(action_tx.clone())?;
        self.navigator
            .register_config_handler(self.config.clone())?;
        self.navigator.init(tui.size()?)?;

        self.firewall.register_action_handler(action_tx.clone())?;
        self.firewall.register_config_handler(self.config.clone())?;
        self.firewall.init(tui.size()?)?;

        // If it's an error, there is no file and thus there is nothing to do.
        self.state.load_packet_filter_rules().await?;
        let filters = self.state.get_filters();
        self.firewall.load_list(filters.to_vec());

        self.filter_form
            .register_action_handler(action_tx.clone())?;
        self.filter_form
            .register_config_handler(self.config.clone())?;
        self.filter_form.init(tui.size()?)?;

        self.profiles.register_action_handler(action_tx.clone())?;
        self.profiles.register_config_handler(self.config.clone())?;
        self.profiles.init(tui.size()?)?;
        self.profiles.get_profile_list_from_storage().await?;

        // Load the state.
        self.load_state().await?;

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
                    debug!("{action:?}");
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
