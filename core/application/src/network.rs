use anyhow::{Context, Result};
use lightning_interfaces::types::Genesis;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Network {
    LocalnetExample,
    TestnetStable,
    TestnetStaging,
}

impl Network {
    pub fn genesis(&self) -> Result<Genesis> {
        let raw = match self {
            Network::LocalnetExample => include_str!("../networks/localnet-example/genesis.toml"),
            Network::TestnetStable => include_str!("../networks/testnet-stable/genesis.toml"),
            Network::TestnetStaging => include_str!("../networks/testnet-staging/genesis.toml"),
        };
        let genesis = toml::from_str(raw).context("Failed to parse genesis file")?;

        Ok(genesis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_localnet_example_genesis() {
        assert!(Network::LocalnetExample.genesis().is_ok());
    }

    #[test]
    fn test_testnet_stable_genesis() {
        assert!(Network::TestnetStable.genesis().is_ok());
    }

    #[test]
    fn test_testnet_staging_genesis() {
        assert!(Network::TestnetStaging.genesis().is_ok());
    }
}
