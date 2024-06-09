use std::collections::HashSet;
use std::net::IpAddr;

use serde::{Deserialize, Serialize};

use crate::{AdminError, FirewallError};

/// The connection policy for the firewall
///
/// This policy can be used to whitelist or blacklist ip addresses
/// and can be used to control who can connect to the endpoint
#[derive(Debug)]
pub enum ConnectionPolicy {
    All,
    Whitelist(HashSet<IpAddr>),
    Blacklist(HashSet<IpAddr>),
}

impl Default for ConnectionPolicy {
    fn default() -> Self {
        Self::All
    }
}

impl ConnectionPolicy {
    pub fn whitelist(members: Vec<IpAddr>) -> Self {
        ConnectionPolicy::Whitelist(members.into_iter().collect())
    }

    pub fn blacklist(members: Vec<IpAddr>) -> Self {
        ConnectionPolicy::Blacklist(members.into_iter().collect())
    }
}

impl ConnectionPolicy {
    pub fn toggle_blacklist(&mut self, ip: IpAddr) -> Result<(), AdminError> {
        tracing::trace!("Toggling blacklist for ip: {:?}", ip);

        match self {
            ConnectionPolicy::Blacklist(blacklist) => {
                if blacklist.contains(&ip) {
                    blacklist.remove(&ip);
                } else {
                    blacklist.insert(ip);
                }
            },
            _ => {
                return Err(AdminError::WrongPolicyType(self.mode()));
            },
        }

        Ok(())
    }

    pub fn toggle_whitelist(&mut self, ip: IpAddr) -> Result<(), AdminError> {
        tracing::trace!("Toggling whitelist for ip: {:?}", ip);

        match self {
            ConnectionPolicy::Whitelist(whitelist) => {
                if whitelist.contains(&ip) {
                    whitelist.remove(&ip);
                } else {
                    whitelist.insert(ip);
                }
            },
            _ => {
                return Err(AdminError::WrongPolicyType(self.mode()));
            },
        }

        Ok(())
    }

    pub fn members(&self) -> Vec<IpAddr> {
        match self {
            ConnectionPolicy::All => Vec::new(),
            ConnectionPolicy::Whitelist(whitelist) => whitelist.iter().cloned().collect(),
            ConnectionPolicy::Blacklist(blacklist) => blacklist.iter().cloned().collect(),
        }
    }

    /// Warning! This will clear the current policy
    pub fn set_policy(&mut self, policy: ConnectionPolicyMode) {
        tracing::trace!("Setting policy to: {:?}", policy);

        *self = match policy {
            ConnectionPolicyMode::All => ConnectionPolicy::All,
            ConnectionPolicyMode::Whitelist => ConnectionPolicy::Whitelist(HashSet::new()),
            ConnectionPolicyMode::Blacklist => ConnectionPolicy::Blacklist(HashSet::new()),
        };
    }
}

impl ConnectionPolicy {
    pub fn check(&self, ip: IpAddr) -> Result<(), FirewallError> {
        match self {
            ConnectionPolicy::All => {},
            ConnectionPolicy::Whitelist(whitelist) => {
                if !whitelist.contains(&ip) {
                    return Err(FirewallError::NotWhitelisted);
                }
            },
            ConnectionPolicy::Blacklist(blacklist) => {
                if blacklist.contains(&ip) {
                    return Err(FirewallError::Blacklisted);
                }
            },
        };

        Ok(())
    }
}

impl ConnectionPolicy {
    pub fn mode(&self) -> ConnectionPolicyMode {
        match self {
            ConnectionPolicy::All => ConnectionPolicyMode::All,
            ConnectionPolicy::Whitelist(_) => ConnectionPolicyMode::Whitelist,
            ConnectionPolicy::Blacklist(_) => ConnectionPolicyMode::Blacklist,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum ConnectionPolicyMode {
    All,
    Whitelist,
    Blacklist,
}
