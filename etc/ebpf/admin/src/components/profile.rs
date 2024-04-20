mod forms;
mod view;

use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

use color_eyre::eyre::Result;
use color_eyre::owo_colors::OwoColorize;
use color_eyre::Report;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ebpf_service::map::{self, PacketFilterRule};
use ebpf_service::ConfigSource;
use log::error;
use ratatui::prelude::{Constraint, Layout, Modifier, Rect, Style};
use ratatui::widgets::{Block, Borders};
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;
use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::{Input, TextArea};
use unicode_width::UnicodeWidthStr;

use super::{Component, Frame};
use crate::action::Action;
use crate::components::profile::view::ProfileView;
use crate::config::{Config, KeyBindings};
use crate::mode::Mode;
use crate::widgets::list::List;

const COLUMN_COUNT: usize = 6;

#[derive(Default)]
pub struct Profile {
    command_tx: Option<UnboundedSender<Action>>,
    profiles_to_update: Option<Vec<map::Profile>>,
    src: ConfigSource,
    longest_item_per_column: [u16; COLUMN_COUNT],
    list: List<String>,
    profile_view: ProfileView,
    config: Config,
}

impl Profile {
    pub fn new(src: ConfigSource) -> Self {
        let mock_profiles = vec!["~/path/to/bin1".to_string(), "~/path/to/bin1".to_string()];

        Self {
            src,
            profiles_to_update: None,
            command_tx: None,
            longest_item_per_column: [0; COLUMN_COUNT],
            list: List::with_records(mock_profiles, "Profiles"),
            profile_view: ProfileView::new(),
            config: Config::default(),
        }
    }

    pub async fn get_profile_list_from_storage(&mut self) -> Result<()> {
        // If it's an error, there are no files and thus there is nothing to do.
        if let Ok(profiles) = self
            .src
            .get_profiles()
            .await
            .map_err(|e| Report::msg(e.to_string()))
        {
            self.list.update_state(profiles);
        }
        Ok(())
    }

    fn add_profile(&mut self, profile: map::Profile) {
        self.list.add_record(
            profile
                .name
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or("global.json".to_string()),
        );

        if self.profiles_to_update.is_none() {
            self.profiles_to_update = Some(Vec::new());
        }

        let profiles_to_update = self
            .profiles_to_update
            .as_mut()
            .expect("Already initialized");
        profiles_to_update.push(profile);
    }

    fn update_storage(&mut self) {
        if let Some(new) = self.profiles_to_update.take() {
            let command_tx = self
                .command_tx
                .clone()
                .expect("Component always has a sender");
            let storage = self.src.clone();
            tokio::spawn(async move {
                if let Err(e) = storage.write_profiles(new).await {
                    let _ = command_tx.send(Action::Error(e.to_string()));
                }
            });
        }
    }

    fn prefill_view(&mut self) -> Result<()> {
        if let Some(selected) = self.list.get() {
            let profile = self
                .src
                .blocking_read_profile(Some(selected))
                .map_err(|e| Report::msg(e.to_string()))?;
            self.profile_view.update_state(profile);
        }
        Ok(())
    }

    fn empty_view(&mut self) {
        self.profile_view.update_state(map::Profile::default());
    }

    pub fn view(&mut self) -> &mut ProfileView {
        &mut self.profile_view
    }
}

impl Component for Profile {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx.clone());
        self.profile_view.register_action_handler(tx)
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config.clone();
        self.profile_view.register_config_handler(config)
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Edit => Ok(Some(Action::UpdateMode(Mode::ProfilesEdit))),
            Action::Save => Ok(Some(Action::UpdateMode(Mode::Profiles))),
            Action::Cancel => Ok(None),
            Action::Add => {
                self.empty_view();
                Ok(Some(Action::UpdateMode(Mode::ProfileViewEdit)))
            },
            Action::Remove => Ok(None),
            Action::Up => {
                self.list.scroll_up();
                Ok(Some(Action::Render))
            },
            Action::Down => {
                self.list.scroll_down();
                Ok(Some(Action::Render))
            },
            Action::Select => {
                if let Err(e) = self.prefill_view() {
                    return Ok(Some(Action::Error(e.to_string())));
                }
                Ok(Some(Action::UpdateMode(Mode::ProfileView)))
            },
            Action::UpdateMode(Mode::ProfilesEdit) => {
                if let Some(new) = self.profile_view.yank_profile() {
                    self.add_profile(new);
                }
                Ok(None)
            },
            _ => Ok(None),
        }
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        self.list.render(f, area)
    }
}
