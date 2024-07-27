mod forms;
mod view;

use std::collections::HashSet;
use std::str::FromStr;

use anyhow::Result;
use forms::RuleForm;
use lightning_guard::map::FileRule;
use lightning_guard::{map, ConfigSource};
use ratatui::prelude::Rect;

use super::prompt::PromptChange;
use super::{Component, Draw, Frame};
use crate::app::{ApplicationContext, GlobalAction};
use crate::components::profile::forms::ProfileForm;
use crate::components::profile::view::ProfileView;
use crate::config::{ComponentKeyBindings, Config};
use crate::helpers::StableVec;
use crate::widgets::context_list::ContextList;

pub struct ProfileContext {
    /// The current mounted subcomponent
    mounted: ProfileSubComponent,
    profiles: StableVec<map::Profile>,
    view_profile: Option<(usize, StableVec<FileRule>)>,
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
    list: ContextList<ProfileContext, map::Profile>,
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
            list: ContextList::new("Profiles", |context: &mut ProfileContext| &mut context.profiles),
            view: ProfileView::new(src),
            form: ProfileForm::new(),
            rule_form: RuleForm::new(),
            keybindings: Default::default(),
            context: ProfileContext {
                mounted: ProfileSubComponent::Profiles,
                profiles: StableVec::new(),
                view_profile: None,
            },
        };

        this.get_profile_list_from_storage().await?;

        Ok(this)
    }

    pub async fn get_profile_list_from_storage(&mut self) -> Result<()> {
        // If it's an error, there are no files and thus there is nothing to do.
        if let Ok(profiles) = self.src.get_profiles().await {
            self.list.add_records_and_commit(&mut self.context, profiles);
        }

        Ok(())
    }

    fn save(&mut self) {
        let remove = self
            .list
            .commit_changes(&mut self.context)
            .into_iter()
            .map(|p| p.name)
            .collect::<HashSet<_>>();

        let update = self.list.uncommitted(&mut self.context).to_vec();
        let storage = self.src.clone();
        tokio::spawn(async move {
            // Todo: do better.
            if let Err(e) = storage.delete_profiles(remove).await {
                log::error!("Failed to delete profiles: {:?}", e);
            }

            if !update.is_empty() {
                if let Err(e) = storage.write_profiles(update).await {
                    log::error!("Failed to delete profiles: {:?}", e);
                }
            }
        });
    }

    fn load_profile_into_view(&mut self) -> Result<()> {
        if let Some(selected) = self.list.selected() {
            let profile = self.src.blocking_read_profile(
                self.list
                    .records(&mut self.context)
                    .get(selected)
                    .expect("a valid selected idx")
                    .name
                    .as_ref()
                    .and_then(|name| name.file_stem()),
            )?;

            self.context.view_profile = Some((selected, profile.file_rules.into()));
        }
        
        Ok(())
    }

    fn restore_state(&mut self) {
        self.list.restore_state(&mut self.context);
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
                self.list.draw(&mut self.context, f, area)
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

        self.keybindings
            .extend(config.keybindings.parse_actions(self.component_name()));
        self.keybindings
            .extend(config.keybindings.parse_actions("ProfilesEdit"));
    }

    fn handle_event(
        &mut self,
        context: &mut Self::Context,
        event: &[crossterm::event::KeyEvent],
    ) -> Result<Option<crate::app::GlobalAction>> {
        let maybe_action = match self.context.mounted {
            ProfileSubComponent::ProfileForm => self.form.handle_event(&mut self.context, event),
            ProfileSubComponent::ProfileView | ProfileSubComponent::ProfileViewEdit => {
                self.view.handle_event(&mut self.context, event)
            },
            ProfileSubComponent::ProfileRuleForm => {
                self.rule_form.handle_event(&mut self.context, event)
            },
            ProfileSubComponent::Profiles | ProfileSubComponent::ProfilesEdit => {
                if let Some(action) = self.keybindings.get(event) {
                    match action {
                        ProfileAction::Add => {
                            self.context.mounted = ProfileSubComponent::ProfileForm;
                        },
                        ProfileAction::Edit => {
                            self.context.mounted = ProfileSubComponent::ProfilesEdit;
                        },
                        ProfileAction::Remove => {
                            self.list.remove_selected_record(&mut self.context);
                        },
                        ProfileAction::Save => {
                            self.save();
                        },
                        ProfileAction::Cancel => {
                            self.context.mounted = ProfileSubComponent::Profiles;
                            self.restore_state();
                        },
                        ProfileAction::Up => {
                            self.list.scroll_up();
                        },
                        ProfileAction::Down => {
                            self.list.scroll_down(&mut self.context);
                        },
                        ProfileAction::Select => {
                            if let Err(e) = self.load_profile_into_view() {
                                return Err(e.context("Failed to load profile into view"));
                            }

                            self.context.mounted = ProfileSubComponent::ProfileView;
                        },
                        ProfileAction::NavLeft => {
                            context.nav_left();
                        },
                        ProfileAction::NavRight => {
                            context.nav_right();
                        },
                        ProfileAction::Quit => return Ok(Some(GlobalAction::Quit)),
                        _ => {},
                    }

                    Ok(Some(GlobalAction::Render))
                } else {
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
}
