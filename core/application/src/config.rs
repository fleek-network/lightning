use std::time::SystemTime;

use anyhow::{anyhow, Result};
use lightning_utils::config::LIGHTNING_HOME_DIR;
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};

use crate::genesis::Genesis;
use crate::network::Network;

#[derive(Clone, Serialize, Deserialize, Default)]
pub enum Mode {
    #[default]
    Dev,
    Test,
    Prod,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub network: Option<Network>,
    pub genesis: Option<Genesis>,
    pub mode: Mode,
    pub testnet: bool,
    pub storage: StorageConfig,
    pub db_path: Option<ResolvedPathBuf>,
    pub db_options: Option<ResolvedPathBuf>,
}

impl Config {
    pub fn test() -> Self {
        Self {
            network: None,
            genesis: Some(Genesis::default()),
            mode: Mode::Test,
            testnet: false,
            storage: StorageConfig::InMemory,
            db_path: None,
            db_options: None,
        }
    }

    pub fn genesis(&self) -> Result<Genesis> {
        let mut genesis = match &self.network {
            Some(network) => match &self.genesis {
                Some(_genesis) => Err(anyhow!("Cannot specify both network and genesis in config")),
                None => network.genesis(),
            },
            None => match &self.genesis {
                Some(genesis) => Ok(genesis.clone()),
                None => Err(anyhow!("Missing network in config")),
            },
        }?;
        if let Mode::Dev = self.mode {
            genesis.epoch_start = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
        }
        Ok(genesis)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            network: Some(Network::LocalnetExample),
            genesis: None,
            mode: Mode::Dev,
            testnet: true,
            storage: StorageConfig::RocksDb,
            db_path: Some(
                LIGHTNING_HOME_DIR
                    .join("data/app_db")
                    .try_into()
                    .expect("Failed to resolve path"),
            ),
            db_options: None,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum StorageConfig {
    InMemory,
    RocksDb,
}

#[cfg(test)]
mod config_tests {
    use super::*;

    #[test]
    fn genesis_with_network_without_genesis() {
        let config = Config {
            network: Some(Network::LocalnetExample),
            genesis: None,
            ..Config::default()
        };
        assert!(config.genesis().is_ok());
    }

    #[test]
    fn genesis_without_network_with_genesis() {
        let config = Config {
            network: None,
            genesis: Some(Genesis::default()),
            ..Config::default()
        };
        assert!(config.genesis().is_ok());
    }

    #[test]
    fn genesis_missing_network_and_genesis() {
        let config = Config {
            network: None,
            genesis: None,
            ..Config::default()
        };
        assert!(config.genesis().is_err());
    }

    #[test]
    fn genesis_with_network_and_genesis() {
        let config = Config {
            network: Some(Network::LocalnetExample),
            genesis: Some(Genesis::default()),
            ..Config::default()
        };
        assert!(config.genesis().is_err());
    }
}
