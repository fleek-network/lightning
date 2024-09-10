mod forms;
mod view;

use anyhow::Result;
pub use forms::{ProfileForm, RuleForm};
use lightning_guard::map;
use ratatui::prelude::Rect;
use tokio::sync::mpsc::UnboundedSender;
pub use view::ProfileView;

use super::{Component, Frame};
use crate::action::Action;
use crate::components::utils::list::List;
use crate::config::Config;
use crate::mode::Mode;
use crate::state::State;

/// Component that displaying and managing security profiles.
pub struct Profile {
    command_tx: Option<UnboundedSender<Action>>,
    list: List<map::Profile>,
    config: Config,
}

impl Profile {
    pub fn new() -> Self {
        Self {
            command_tx: None,
            list: List::new("Profiles"),
            config: Config::default(),
        }
    }

    pub fn update_profiles(&mut self, profiles: Vec<map::Profile>) {
        self.list.load_records(profiles);
    }

    fn restore_state(&mut self) {
        self.list.restore_state();
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
                // It is possible that the user deleted some entries thus we handle that here.
                let remove = self
                    .list
                    .records_to_remove_mut()
                    .map(|p| p.name.clone())
                    .collect();
                ctx.commit_remove_profiles(remove);

                self.list.commit_changes();
                let profiles = self.list.records();
                ctx.update_profiles(profiles);
                ctx.commit_add_profiles();

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
                    ctx.select_profile(profile);
                    Ok(Some(Action::UpdateMode(Mode::ProfileView)))
                } else {
                    Ok(None)
                }
            },
            Action::UpdateMode(Mode::ProfilesEdit) => {
                // We moved from a different mode
                // and we assume that the current state is valid
                // so we display what's in state.
                let profiles = ctx.get_profiles();
                self.list.clear();
                self.update_profiles(profiles.to_vec());
                Ok(None)
            },
            _ => Ok(None),
        }
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        self.list.render(f, area)
    }
}
