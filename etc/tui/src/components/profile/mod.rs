mod forms;
mod view;

use std::collections::HashSet;
use std::str::FromStr;

use anyhow::Result;
use forms::RuleForm;
use lightning_guard::{map, ConfigSource};
use ratatui::prelude::Rect;
use view::ProfileViewContext;

use super::prompt::PromptChange;
use super::{Component, Draw, Frame};
use crate::app::{ApplicationContext, GlobalAction};
use crate::components::profile::forms::ProfileForm;
use crate::components::profile::view::ProfileView;
use crate::config::{ComponentKeyBindings, Config};
use crate::widgets::list::List;
use crate::widgets::table::Table;

pub struct ProfileContext {
    /// The current mounted subcomponent
    mounted: ProfileSubComponent,
    profiles_to_update: Option<Vec<map::Profile>>,
    list: List<map::Profile>,
    profile_view_context: ProfileViewContext,
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
    ProfilesEdit,
    /// Adding a new a profile
    ProfileForm,
    /// Info about a specific profile
    ProfileView,
    ProfileViewEdit,
    /// Adding a rule to a profile
    ProfileRuleForm,
}

impl ProfileSubComponent {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Profiles => "Profiles",
            Self::ProfileForm => "ProfileForm",
            Self::ProfileView => "ProfileView",
            Self::ProfileViewEdit => "ProfileViewEdit",
            Self::ProfilesEdit => "ProfilesEdit",
            Self::ProfileRuleForm => "ProfileRuleForm",
        }
    }
}

/// Component that displaying and managing security profiles.
pub struct Profile {
    src: ConfigSource,
    view: ProfileView,
    form: ProfileForm,
    rule_form: RuleForm,
    context: ProfileContext,
    keybindings: ComponentKeyBindings<ProfileAction>,
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
    NavLeft,
    NavRight,
    Suspend,
    Quit,
}

impl FromStr for ProfileAction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Edit" => Ok(Self::Edit),
            "Save" => Ok(Self::Save),
            "Cancel" => Ok(Self::Cancel),
            "Add" => Ok(Self::Add),
            "Remove" => Ok(Self::Remove),
            "Up" => Ok(Self::Up),
            "Down" => Ok(Self::Down),
            "Select" => Ok(Self::Select),
            "NavLeft" => Ok(Self::NavLeft),
            "NavRight" => Ok(Self::NavRight),
            "Suspend" => Ok(Self::Suspend),
            "Quit" => Ok(Self::Quit),
            _ => Err(anyhow::anyhow!("Invalid action")),
        }
    }
}

impl Profile {
    pub async fn new(src: ConfigSource) -> Result<Self> {
        let mut this = Self {
            src: src.clone(),
            view: ProfileView::new(src),
            form: ProfileForm::new(),
            rule_form: RuleForm::new(),
            keybindings: Default::default(),
            context: ProfileContext {
                mounted: ProfileSubComponent::Profiles,
                profiles_to_update: None,
                list: List::new("Profiles"),
                profile_view_context: ProfileViewContext {
                    table: Table::new(),
                    profile: None,
                },
            },
        };

        this.get_profile_list_from_storage().await?;

        Ok(this)
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
            .context
            .list
            .records_to_remove_mut()
            .map(|profile| profile.name.take())
            .collect::<HashSet<_>>();

        self.context.list.commit_changes();

        let update = self.context.profiles_to_update.take();
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
        if let Some(selected) = self.context.list.get() {
            let profile = self
                .src
                .blocking_read_profile(selected.name.as_ref().and_then(|name| name.file_stem()))?;

            self.context.profile_view_context.load_profile(profile);
        }
        Ok(())
    }

    fn restore_state(&mut self) {
        self.context.list.restore_state();
        self.context.profiles_to_update.take();
    }
}

impl Draw for Profile {
    type Context = ApplicationContext;

    fn draw(&mut self, _context: &mut Self::Context, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        match self.context.mounted {
            ProfileSubComponent::ProfileForm => self.form.draw(&mut self.context, f, area),
            ProfileSubComponent::ProfileView | ProfileSubComponent::ProfileViewEdit => {
                self.view.draw(&mut self.context, f, area)
            },
            ProfileSubComponent::ProfileRuleForm => self.rule_form.draw(&mut self.context, f, area),
            ProfileSubComponent::Profiles | ProfileSubComponent::ProfilesEdit => {
                self.context.list.render(f, area)
            },
        }
    }
}

impl Component for Profile {
    fn component_name(&self) -> &'static str {
        "Profiles"
    }

    fn register_keybindings(&mut self, config: &Config) {
        let _ = self.view.register_keybindings(config);
        let _ = self.form.register_keybindings(config);
        let _ = self.rule_form.register_keybindings(config);

        // todo this works because the keybinds dont have any overlap
        let main_bindings =
            crate::config::parse_actions(&config.keybindings[self.component_name()]);
        let edit_bindinsg = crate::config::parse_actions(&config.keybindings["ProfilesEdit"]);

        self.keybindings.extend(main_bindings);
        self.keybindings.extend(edit_bindinsg);
    }

    fn handle_known_event(
        &mut self,
        context: &mut Self::Context,
        event: &[crossterm::event::KeyEvent],
    ) -> Result<Option<crate::app::GlobalAction>> {
        let maybe_action = match self.context.mounted {
            ProfileSubComponent::ProfileForm => {
                self.form.handle_known_event(&mut self.context, event)
            },
            ProfileSubComponent::ProfileView | ProfileSubComponent::ProfileViewEdit => {
                self.view.handle_known_event(&mut self.context, event)
            },
            ProfileSubComponent::ProfileRuleForm => {
                self.rule_form.handle_known_event(&mut self.context, event)
            },
            ProfileSubComponent::Profiles | ProfileSubComponent::ProfilesEdit => {
                if let Some(action) = self.keybindings.get(event) {
                    match action {
                        ProfileAction::Add => {
                            self.context.mounted = ProfileSubComponent::ProfileForm;
                            Ok(Some(crate::app::GlobalAction::Render))
                        },
                        ProfileAction::Edit => {
                            self.context.mounted = ProfileSubComponent::ProfilesEdit;
                            Ok(Some(crate::app::GlobalAction::Render))
                        },
                        ProfileAction::Remove => {
                            self.context.list.remove_selected_record();
                            Ok(Some(crate::app::GlobalAction::Render))
                        },
                        ProfileAction::Save => {
                            self.save();
                            Ok(None)
                        },
                        ProfileAction::Cancel => {
                            self.context.mounted = ProfileSubComponent::Profiles;
                            self.restore_state();
                            Ok(None)
                        },
                        ProfileAction::Up => {
                            self.context.list.scroll_up();
                            Ok(Some(crate::app::GlobalAction::Render))
                        },
                        ProfileAction::Down => {
                            self.context.list.scroll_down();
                            Ok(Some(crate::app::GlobalAction::Render))
                        },
                        ProfileAction::Select => {
                            if let Err(e) = self.load_profile_into_view() {
                                return Err(e.context("Failed to load profile into view"));
                            }

                            self.context.mounted = ProfileSubComponent::ProfileView;
                            Ok(Some(crate::app::GlobalAction::Render))
                        },
                        ProfileAction::NavLeft => {
                            context.nav_left();

                            Ok(Some(crate::app::GlobalAction::Render))
                        },
                        ProfileAction::NavRight => {
                            context.nav_right();

                            Ok(Some(crate::app::GlobalAction::Render))
                        },
                        ProfileAction::Quit => Ok(Some(GlobalAction::Quit)),
                        _ => Ok(None),
                    }
                } else {
                    log::error!("Unknown event: {:?}", event);
                    log::error!(
                        "This should have been prevalidated before being passed to the component."
                    );
                    Ok(None)
                }
            },
        };

        // if the top level profiles is mounted, 
        // lets make sure we track any subcompomnent prompts
        if !context.component_changed() {
            context.change_prompt(PromptChange::Change(self.context.mounted.as_str()));
        }

        maybe_action
    }

    fn is_known_event(&self, event: &[crossterm::event::KeyEvent]) -> bool {
        match self.context.mounted {
            ProfileSubComponent::ProfileForm => self.form.is_known_event(event),
            ProfileSubComponent::ProfileRuleForm => self.rule_form.is_known_event(event),
            ProfileSubComponent::ProfileView | ProfileSubComponent::ProfileViewEdit => {
                self.view.is_known_event(event)
            },
            ProfileSubComponent::Profiles | ProfileSubComponent::ProfilesEdit => {
                self.keybindings.contains_key(event)
            },
        }
    }
}
