mod forms;
mod view;

use std::collections::HashSet;

use anyhow::Result;
pub use forms::{ProfileForm, RuleForm};
use lightning_guard::{map, ConfigSource};
use ratatui::prelude::Rect;
use tokio::sync::mpsc::UnboundedSender;
pub use view::ProfileView;

use super::{Component, Frame};
use crate::action::Action;
use crate::config::Config;
use crate::mode::Mode;
use crate::state::State;
use crate::widgets::list::List;

/// Component that displaying and managing security profiles.
pub struct Profile {
    command_tx: Option<UnboundedSender<Action>>,
    profiles_to_update: Option<Vec<map::Profile>>,
    src: ConfigSource,
    list: List<map::Profile>,
    config: Config,
}

impl Profile {
    pub fn new(src: ConfigSource) -> Self {
        Self {
            src: src.clone(),
            profiles_to_update: None,
            command_tx: None,
            list: List::new("Profiles"),
            config: Config::default(),
        }
    }

    pub fn update_profiles(&mut self, profiles: Vec<map::Profile>) {
        self.list.load_records(profiles);
    }

    fn add_profile(&mut self, profile: map::Profile) {
        self.list.add_record(profile.clone());

        if self.profiles_to_update.is_none() {
            self.profiles_to_update = Some(Vec::new());
        }

        let profiles_to_update = self
            .profiles_to_update
            .as_mut()
            .expect("Already initialized");
        profiles_to_update.push(profile);
    }

    fn get_selected_profile(&mut self) -> Option<map::Profile> {
        self.list.get().map(Clone::clone)
    }

    fn restore_state(&mut self) {
        self.list.restore_state();
        self.profiles_to_update.take();
    }
}

impl Component for Profile {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx.clone());
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config.clone();
        Ok(())
    }

    fn update(&mut self, action: Action, ctx: &mut State) -> Result<Option<Action>> {
        match action {
            Action::Edit => Ok(Some(Action::UpdateMode(Mode::ProfilesEdit))),
            Action::Save => {
                ctx.commit_profiles();
                Ok(Some(Action::UpdateMode(Mode::Profiles)))
            },
            Action::Cancel => {
                self.restore_state();
                Ok(Some(Action::UpdateMode(Mode::Profiles)))
            },
            Action::Add => Ok(Some(Action::UpdateMode(Mode::ProfileForm))),
            Action::Remove => {
                self.list.remove_selected_record();
                Ok(Some(Action::Render))
            },
            Action::Up => {
                self.list.scroll_up();
                Ok(Some(Action::Render))
            },
            Action::Down => {
                self.list.scroll_down();
                Ok(Some(Action::Render))
            },
            Action::Select => {
                // Todo: We should use some type of unique identification
                // to maintain consistency.
                if let Some(profile) = self.list.get() {
                    ctx.select_profile(profile.clone());
                }
                Ok(Some(Action::UpdateMode(Mode::ProfileView)))
            },
            Action::UpdateMode(Mode::ProfilesEdit) => {
                let profiles = ctx.get_profiles();
                self.update_profiles(profiles.to_vec());
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        self.list.render(f, area)
    }
}
