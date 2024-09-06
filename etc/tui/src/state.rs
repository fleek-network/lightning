use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use lightning_guard::map::{FileRule, PacketFilterRule, Profile};
use lightning_guard::ConfigSource;
use log::error;

pub struct State {
    filters: Vec<PacketFilterRule>,
    profiles: HashMap<Option<PathBuf>, Profile>,
    selected_profile: Option<PathBuf>,
    src: ConfigSource,
}

impl State {
    pub fn new(src: ConfigSource) -> Self {
        Self {
            filters: Vec::new(),
            profiles: HashMap::new(),
            selected_profile: None,
            src,
        }
    }

    pub async fn load_filters(&mut self) -> Result<()> {
        self.filters = self.src.read_packet_filters().await?;
        Ok(())
    }

    pub fn add_filters(&mut self, filters: Vec<PacketFilterRule>) {
        self.filters = filters;
    }

    pub fn add_filter(&mut self, filter: PacketFilterRule) {
        self.filters.push(filter);
    }

    pub fn commit_filters(&mut self) {
        let filters = self.filters.clone();
        let src = self.src.clone();
        tokio::spawn(async move {
            if let Err(e) = src.write_packet_filters(filters).await {
                error!("failed to write profiles to disk: {e:?}");
            }
        });
    }

    pub fn get_filters(&self) -> &[PacketFilterRule] {
        self.filters.as_slice()
    }

    pub async fn load_profiles(&mut self) -> Result<()> {
        self.profiles = self
            .src
            .get_profiles()
            .await?
            .into_iter()
            .map(|p| (p.name.clone(), p))
            .collect();
        Ok(())
    }

    pub fn add_profile(&mut self, profiles: Profile) {
        self.profiles.insert(profiles.name.clone(), profiles);
    }

    pub fn commit_add_profiles(&mut self) {
        let profiles = self.profiles.clone().into_iter().map(|(_, p)| p).collect();
        let src = self.src.clone();
        tokio::spawn(async move {
            if let Err(e) = src.write_profiles(profiles).await {
                error!("failed to write profiles to disk: {e:?}");
            }
        });
    }

    pub fn commit_remove_profiles(&mut self, remove: Vec<Option<PathBuf>>) {
        let src = self.src.clone();
        tokio::spawn(async move {
            if let Err(e) = src.delete_profiles(remove.into_iter().collect()).await {
                error!("failed to write profiles to disk: {e:?}");
            }
        });
    }

    pub fn get_profiles(&self) -> Vec<Profile> {
        self.profiles.values().cloned().collect()
    }

    pub fn update_selected_profile_rules_list(&mut self, rules: Vec<FileRule>) {
        let name = &self.selected_profile;
        let profile = self.profiles.get_mut(name).expect("Profile to exist");
        profile.file_rules = rules;
    }

    pub fn get_profile_rules(&self, name: &Option<PathBuf>) -> &[FileRule] {
        self.profiles
            .get(name)
            .expect("There to be a profile")
            .file_rules
            .as_slice()
    }

    pub fn get_selected_profile(&self) -> Option<&Profile> {
        self.profiles.get(&self.selected_profile)
    }

    pub fn get_selected_profile_mut(&mut self) -> Option<&mut Profile> {
        self.profiles.get_mut(&self.selected_profile)
    }

    pub fn select_profile(&mut self, profile: &Profile) {
        self.selected_profile = profile.name.clone();
        debug_assert!(self.profiles.contains_key(&self.selected_profile));
    }

    pub fn update_profiles(&mut self, profiles: Vec<Profile>) {
        self.profiles = profiles.into_iter().map(|p| (p.name.clone(), p)).collect();
    }
}
