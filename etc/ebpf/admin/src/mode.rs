use std::io;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Home,
    Firewall,
    FirewallNewEntry,
    Profiles,
}

impl FromStr for Mode {
    type Err = io::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mode = match value {
            "Firewall" => Mode::Firewall,
            "Home" => Mode::Home,
            "Profiles" => Mode::Profiles,
            _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid mode")),
        };

        Ok(mode)
    }
}
