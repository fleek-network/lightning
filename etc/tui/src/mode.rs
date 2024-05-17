use std::io;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Home,
    Firewall,
    FirewallEdit,
    FirewallForm,
    Logger,
    Profiles,
    ProfilesEdit,
    ProfileForm,
    ProfileView,
    ProfileViewEdit,
    ProfileRuleForm,
}

impl FromStr for Mode {
    type Err = io::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mode = match value {
            "Home" => Mode::Home,
            "Firewall" => Mode::Firewall,
            "Logger" => Mode::Logger,
            "Profiles" => Mode::Profiles,
            _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid mode")),
        };

        Ok(mode)
    }
}
