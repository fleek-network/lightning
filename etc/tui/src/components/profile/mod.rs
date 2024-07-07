mod forms;
mod view;

use std::collections::HashSet;
use std::fmt::Display;

use anyhow::Result;
use lightning_guard::{map, ConfigSource};
use ratatui::prelude::Rect;
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, Draw, Frame};
use crate::app::ApplicationContext;
use crate::components::profile::forms::ProfileForm;
use crate::components::profile::view::ProfileView;
use crate::config::{ComponentKeyBindings, Config};
use crate::widgets::list::List;

pub struct ProfileContext {
    /// The current mounted subcomponent
    mounted: ProfileSubComponent,
    profiles_to_update: Option<Vec<map::Profile>>,
    list: List<map::Profile>,
}

impl ProfileContext {
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
}

#[derive(Debug, Clone, Copy)]
pub enum ProfileSubComponent {
    /// The list of profiles
    Profiles,
    /// Editiing a profile
    ProfileForm,
    /// Info about a specific profile
    ProfileView,
    /// Editing the list of profiles
    ProfileViewEdit,
}

impl ProfileSubComponent {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Profiles => "Profile",
            Self::ProfileForm => "ProfileForm",
            Self::ProfileView => "ProfileView",
            Self::ProfileViewEdit => "ProfileViewEdit",
        }
    }
}

/// Component that displaying and managing security profiles.
pub struct Profile {
    src: ConfigSource,
    view: ProfileView,
    form: ProfileForm,
    context: ProfileContext,
    keybindings: ComponentKeyBindings<ProfileAction>
}

enum ProfileAction {
    Edit,
    Save,
    Cancel,
    Add,
    Remove,
    Up,
    Down,
    Select,
}

impl Profile {
    pub fn new(src: ConfigSource) -> Self {
        Self {
            src: src.clone(),
            view: ProfileView::new(src),
            form: ProfileForm::new(),
            keybindings: Default::default(),
            context: ProfileContext {
                mounted: ProfileSubComponent::Profiles,
                profiles_to_update: None,
                list: List::new("Profiles"),
            },
        }
    }

    pub async fn get_profile_list_from_storage(&mut self) -> Result<()> {
        // If it's an error, there are no files and thus there is nothing to do.
        if let Ok(profiles) = self.src.get_profiles().await {
            self.context.list.load_records(profiles);
        }
        
        Ok(())
    }

    fn save(&mut self) {
        // Remove names of profiles that need to be deleted before they're gone forever.
        let remove = self
            .list
            .records_to_remove_mut()
            .map(|profile| profile.name.take())
            .collect::<HashSet<_>>();

        self.list.commit_changes();

        let update = self.profiles_to_update.take();
        let storage = self.src.clone();
        tokio::spawn(async move {
            // Todo: do better.
            if let Err(e) = storage.delete_profiles(remove).await {
                log::error!("Failed to delete profiles: {:?}", e);
            }

            if let Some(profiles) = update {
                if let Err(e) = storage.write_profiles(profiles).await {
                    log::error!("Failed to delete profiles: {:?}", e);
                }
            }
        });
    }

    fn load_profile_into_view(&mut self) -> Result<()> {
        if let Some(selected) = self.list.get() {
            let profile = self
                .src
                .blocking_read_profile(selected.name.as_ref().and_then(|name| name.file_stem()))?;

            self.view.load_profile(profile);
        }
        Ok(())
    }

    fn restore_state(&mut self) {
        self.list.restore_state();
        self.profiles_to_update.take();
    }
}

impl Draw for Profile {
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        self.list.render(f, area)
    }
}

impl Component for Profile {
    type Context = ApplicationContext;

    fn component_name(&self) -> &'static str {
        "Profile"
    }

    fn register_keybindings(&mut self, config: &Config) {
        self.view.register_keybindings(config);
        self.form.register_keybindings(config);

        crate::config::parse_actions(&config.keybindings[self.component_name()]);
    }

    fn handle_known_event(
        &mut self,
        context: &mut Self::Context,
        event: &[crossterm::event::KeyEvent],
    ) -> Result<Option<crate::app::GlobalAction>> {
        match self.context.mounted {
            ProfileSubComponent::ProfileForm => self.form.handle_event(&mut self.context, event),
            ProfileSubComponent::ProfileView => self.view.handle_event(&mut self.context, event),
            ProfileSubComponent::ProfileViewEdit => self.view.handle_event(&mut self.context, event),
            ProfileSubComponent::Profiles => {
                if let Some(action) = self.keybindings.get(event) {
                    match action {
                        ProfileAction::Add => {
                            self.context.mounted = ProfileSubComponent::ProfileForm;
                            Ok(Some(crate::app::GlobalAction::Render))
                        },
                        ProfileAction::Edit => {
                            self.context.mounted = ProfileSubComponent::ProfileViewEdit;
                            Ok(Some(crate::app::GlobalAction::Render))
                        },
                        ProfileAction::Remove => {
                            self.list.remove_selected_record();
                            Ok(Some(crate::app::GlobalAction::Render))
                        },
                        ProfileAction::Save => {
                            self.save();
                        },
                        ProfileAction::Cancel => {
                            self.restore_state();
                        },
                        ProfileAction::Up => {
                            self.list.scroll_up();
                            Ok(Some(crate::app::GlobalAction::Render))
                        },
                        ProfileAction::Down => {
                            self.list.scroll_down();
                            Ok(Some(crate::app::GlobalAction::Render))
                        },
                        ProfileAction::Select => {
                            if let Err(e) = self.load_profile_into_view() {
                                log::debug!("Error loading profile into view: {:?}", e);
                            }

                            self.context.mounted = ProfileSubComponent::ProfileView;
                            Ok(Some(crate::app::GlobalAction::Render))
                        },
                    }
                }
            }
        }
    }

    fn is_known_event(&self, event: &[crossterm::event::KeyEvent]) -> bool {
        match self.context.mounted {
            ProfileSubComponent::ProfileForm => self.form.is_known_event(event),
            ProfileSubComponent::ProfileView => self.view.is_known_event(event),
            ProfileSubComponent::ProfileViewEdit => self.view.is_known_event(event),
            ProfileSubComponent::Profiles => {
                self.keybindings.get(event).is_some()
            }
        }
    }
}
