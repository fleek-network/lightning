use anyhow::Result;
use lightning_guard::map::{FileRule, PacketFilterRule, Profile};
use lightning_guard::ConfigSource;
use log::error;

use crate::action::Action;

pub struct State {
    next_action: Option<Action>,
    filters: Vec<PacketFilterRule>,
    profiles: Vec<Profile>,
    selected_profile: Option<Profile>,
    src: ConfigSource,
}

impl State {
    pub fn new(src: ConfigSource) -> Self {
        Self {
            next_action: None,
            filters: Vec::new(),
            profiles: Vec::new(),
            selected_profile: None,
            src,
        }
    }

    pub async fn load_packet_filter_rules(&mut self) -> Result<()> {
        self.filters = self.src.read_packet_filters().await?;
        Ok(())
    }

    pub fn update_packet_filters(&mut self, filters: Vec<PacketFilterRule>) {
        self.filters = filters;
    }

    pub fn update_filters(&mut self, filter: PacketFilterRule) {
        self.filters.push(filter);
    }

    pub async fn commit_packet_filters(&mut self) -> Result<()> {
        self.src.write_packet_filters(self.filters.clone()).await
    }

    pub fn get_filters(&self) -> &[PacketFilterRule] {
        self.filters.as_slice()
    }

    pub async fn load_profiles(&mut self) -> Result<()> {
        self.profiles = self.src.get_profiles().await?;
        Ok(())
    }

    pub fn update_profiles(&mut self, profiles: Profile) {
        self.profiles.push(profiles);
    }

    pub fn commit_profiles(&mut self) {
        let profiles = self.profiles.clone();
        let src = self.src.clone();
        tokio::spawn(async move {
            if let Err(e) = src.write_profiles(profiles).await {
                error!("failed to write profiles to disk: {e:?}");
            }
        });
    }

    pub fn get_profiles(&self) -> &[Profile] {
        self.profiles.as_slice()
    }

    /// Note: Panics if profile with `name` does not exist in state.
    pub fn update_profile_rules(&mut self, name: String, rule: FileRule) {
        let profile = self
            .profiles
            .iter_mut()
            .find(|p| {
                p.name
                    .as_ref()
                    .map(|buf| buf.display().to_string() == name)
                    .unwrap_or(false)
            })
            .unwrap();
        profile.file_rules.push(rule);
    }

    pub fn get_profile_rules(&self, name: String) -> Vec<FileRule> {
        // Todo: we need to rework the handling of paths.
        match self.profiles.iter().find(|p| {
            p.name
                .as_ref()
                .map(|buf| buf.display().to_string() == name)
                .unwrap_or(false)
        }) {
            None => Vec::new(),
            Some(profile) => profile.file_rules.clone(),
        }
    }

    pub fn get_selected_profile(&self) -> Option<&Profile> {
        self.selected_profile.as_ref()
    }

    pub fn selected_profile(&self) -> Option<String> {
        self.selected_profile
            .as_ref()
            .map(|p| p.name.as_ref())
            .flatten()
            .map(|path| path.display().to_string())
    }

    pub fn select_profile(&mut self, profile: Profile) -> Option<String> {
        self.selected_profile
            .as_ref()
            .map(|p| p.name.as_ref())
            .flatten()
            .map(|path| path.display().to_string())
    }
}
