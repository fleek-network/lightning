use std::path::Path;
use std::time::SystemTime;

use anyhow::{anyhow, Context, Result};
use atomo::{AtomoBuilder, DefaultSerdeBackend};
use atomo_rocks::{Cache as RocksCache, Env as RocksEnv, Options};
use lightning_interfaces::types::Genesis;
use lightning_utils::config::LIGHTNING_HOME_DIR;
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};

use crate::network::Network;
use crate::storage::AtomoStorageBuilder;

#[derive(Clone, Serialize, Deserialize)]
pub struct ApplicationConfig {
    pub network: Option<Network>,
    pub genesis_path: Option<ResolvedPathBuf>,
    pub storage: StorageConfig,
    pub db_path: Option<ResolvedPathBuf>,
    pub db_options: Option<ResolvedPathBuf>,

    // Development options.
    // Should not be used in production, and will likely break your node if you do.
    pub dev: Option<DevConfig>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DevConfig {
    // Whether to update the genesis epoch start to the current time when starting the node.
    pub update_epoch_start_to_now: bool,
}

impl Default for DevConfig {
    fn default() -> Self {
        Self {
            update_epoch_start_to_now: true,
        }
    }
}

impl ApplicationConfig {
    pub fn test(genesis_path: ResolvedPathBuf) -> Self {
        Self {
            network: None,
            genesis_path: Some(genesis_path),
            storage: StorageConfig::InMemory,
            db_path: None,
            db_options: None,
            dev: None,
        }
    }

    pub fn genesis(&self) -> Result<Option<Genesis>> {
        let mut genesis = match &self.network {
            Some(network) => match &self.genesis_path {
                Some(_genesis_path) => {
                    return Err(anyhow!(
                        "Cannot specify both network and genesis_path in config"
                    ));
                },
                None => Some(network.genesis()?),
            },
            None => match &self.genesis_path {
                Some(genesis_path) => Some(Genesis::load_from_file(genesis_path.clone())?),
                None => None,
            },
        };
        if let (Some(genesis), Some(dev)) = (&mut genesis, &self.dev) {
            if dev.update_epoch_start_to_now {
                genesis.epoch_start = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64
            }
        }
        Ok(genesis)
    }

    pub fn atomo_builder<'a>(
        &'a self,
        checkpoint: Option<([u8; 32], &'a [u8], &'a [String])>,
    ) -> Result<AtomoBuilder<AtomoStorageBuilder, DefaultSerdeBackend>> {
        let storage = match self.storage {
            StorageConfig::RocksDb => {
                let db_path = self
                    .db_path
                    .as_ref()
                    .context("db_path must be specified for RocksDb backend")?;
                let mut db_options = if let Some(db_options) = self.db_options.as_ref() {
                    let (options, _) = Options::load_latest(
                        db_options,
                        RocksEnv::new().context("Failed to create rocks db env.")?,
                        false,
                        // TODO(matthias): I set this lru cache size arbitrarily
                        RocksCache::new_lru_cache(100),
                    )
                    .context("Failed to create rocks db options.")?;
                    options
                } else {
                    Options::default()
                };
                db_options.create_if_missing(true);
                db_options.create_missing_column_families(true);
                match checkpoint {
                    Some((hash, checkpoint, extra_tables)) => {
                        AtomoStorageBuilder::new(Some(db_path.as_path()))
                            .with_options(db_options)
                            .from_checkpoint(hash, checkpoint, extra_tables)
                    },
                    None => {
                        AtomoStorageBuilder::new(Some(db_path.as_path())).with_options(db_options)
                    },
                }
            },
            StorageConfig::InMemory => AtomoStorageBuilder::new::<&Path>(None),
        };

        let atomo = AtomoBuilder::<AtomoStorageBuilder, DefaultSerdeBackend>::new(storage);

        Ok(atomo)
    }
}

impl Default for ApplicationConfig {
    fn default() -> Self {
        Self {
            network: None,
            genesis_path: None,
            storage: StorageConfig::RocksDb,
            db_path: Some(
                LIGHTNING_HOME_DIR
                    .join("data/app_db")
                    .try_into()
                    .expect("Failed to resolve path"),
            ),
            db_options: None,
            dev: None,
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
    use std::fs::File;
    use std::io::Write;
    use std::panic;

    use lightning_interfaces::prelude::*;
    use lightning_test_utils::json_config::JsonConfigProvider;
    use lightning_utils::config::TomlConfigProvider;
    use serde_json::json;
    use tempfile::tempdir;

    use super::*;
    use crate::Application;

    partial_node_components!(TomlConfigTestNodeComponents {
        ConfigProviderInterface = TomlConfigProvider<Self>;
        ApplicationInterface = Application<Self>;
    });

    partial_node_components!(JsonConfigTestNodeComponents {
        ConfigProviderInterface = JsonConfigProvider;
        ApplicationInterface = Application<Self>;
    });

    #[test]
    fn config_toml_fails_to_deserialize_unknown_network() {
        let temp_dir = tempdir().unwrap();

        // Write the configuration.
        let config_path = temp_dir.path().join("config.toml");
        let config = r#"
        [application]
        network = "invalid"
        "#;
        let mut file = File::create(&config_path).unwrap();
        file.write_all(config.as_bytes()).unwrap();

        // Load config into the provider.
        let provider =
            TomlConfigProvider::<TomlConfigTestNodeComponents>::load(&config_path).unwrap();

        // Attempt to get the config.
        // This should panic because of the invalid enum variant.
        let result =
            panic::catch_unwind(|| provider.get::<Application<TomlConfigTestNodeComponents>>());
        assert!(result.is_err());
        if let Err(err) = result {
            let panic_message = err
                .downcast_ref::<String>()
                .map(|s| s.as_str())
                .or_else(|| err.downcast_ref::<&str>().copied())
                .unwrap();
            assert_eq!(
                panic_message,
                "Failed to deserialize 'application' config: unknown variant `invalid`, expected `localnet-example` or `testnet-stable`\nin `network`\n"
            )
        }
    }

    #[test]
    fn config_json_fails_to_deserialize_unknown_network() {
        let config_value = json!({
            "application": {
                "network": "invalid"
            }
        });
        let provider: JsonConfigProvider = config_value.into();

        // Attempt to get the config.
        // This should panic because of the invalid enum variant.
        let result = panic::catch_unwind(|| {
            <JsonConfigProvider as ConfigProviderInterface<JsonConfigTestNodeComponents>>::get::<
                Application<JsonConfigTestNodeComponents>,
            >(&provider)
        });
        assert!(result.is_err());
        if let Err(err) = result {
            let panic_message = err
                .downcast_ref::<String>()
                .map(|s| s.as_str())
                .or_else(|| err.downcast_ref::<&str>().copied())
                .unwrap();
            assert_eq!(
                panic_message,
                "invalid value: Error(\"unknown variant `invalid`, expected `localnet-example` or `testnet-stable`\", line: 0, column: 0)"
            )
        }
    }

    #[test]
    fn genesis_with_network_without_genesis() {
        let config = ApplicationConfig {
            network: Some(Network::LocalnetExample),
            genesis_path: None,
            ..Default::default()
        };
        config.genesis().unwrap().unwrap();
    }

    #[test]
    fn genesis_without_network_with_genesis() {
        let temp_dir = tempdir().unwrap();
        let genesis_path = Genesis::default()
            .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
            .unwrap();
        let config = ApplicationConfig {
            network: None,
            genesis_path: Some(genesis_path),
            ..Default::default()
        };
        config.genesis().unwrap().unwrap();
    }

    #[test]
    fn genesis_missing_network_and_genesis() {
        let config = ApplicationConfig {
            network: None,
            genesis_path: None,
            ..Default::default()
        };
        assert!(config.genesis().unwrap().is_none());
    }

    #[test]
    fn genesis_with_network_and_genesis() {
        let temp_dir = tempdir().unwrap();
        let genesis_path = Genesis::default()
            .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
            .unwrap();
        let config = ApplicationConfig {
            network: Some(Network::LocalnetExample),
            genesis_path: Some(genesis_path),
            ..Default::default()
        };
        assert!(config.genesis().is_err());
    }
}
