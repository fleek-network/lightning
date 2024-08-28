use lightning_guard::{ConfigSource, map};
use lightning_guard::map::{FileRule, PacketFilterRule, Profile};
use crate::action::Action;
use anyhow::Result;

pub struct State {
    next_action: Option<Action>,
    filters: Vec<PacketFilterRule>,
    profiles: Vec<Profile>,
    src: ConfigSource,
}

impl State {
    fn new(src: ConfigSource) -> Self {
        Self {
            next_action: None,
            filters: Vec::new(),
            profiles: Vec::new(),
            src,
        }
    }

    fn load_packet_filter_rules(&mut self) -> Result<()> {
        self.filters = self.src.read_packet_filters()?;
        Ok(())
    }

    fn update_packet_filters(&mut self, filters: Vec<PacketFilterRule>) {
        self.filters = filters;
    }

    fn commit_packet_filters(&mut self) -> Result<()> {
        self.src.write_packet_filters(self.filters.clone())
    }

    fn load_profiles(&mut self) -> Result<()> {
        self.profiles = self.src.get_profiles()?;
        Ok(())
    }

    fn update_profiles(&mut self, profiles: Vec<Profile>) {
        self.profiles = profiles;
    }

    fn commit_profiles(&mut self) -> Result<()> {
        self.src.write_profiles(self.profiles.clone())
    }

    /// Note: Panics if profile with `name` does not exist in state.
    fn update_profile_rules(&mut self, name: String, rules: Vec<FileRule>) {
        let profile = self.profiles.iter_mut()
            .find(|p| p.name.as_ref().map(|buf| buf.display().to_string() == name)
                .unwrap_or(false)).unwrap();
        profile.file_rules =  rules;
    }

    fn get_profile_rules(&self, name: String) -> Vec<FileRule> {
        // Todo: we need to rework the handling of paths.
        match self.profiles.iter()
            .find(|p| p.name.as_ref().map(|buf| buf.display().to_string() == name)
                .unwrap_or(false)) {
            None => Vec::new(),
            Some(profile) => {
                profile.file_rules.clone()
            }
        }
    }
}
