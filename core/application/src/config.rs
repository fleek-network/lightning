use lightning_utils::config::LIGHTNING_HOME_DIR;
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};

use crate::genesis::Genesis;

#[derive(Clone, Serialize, Deserialize, Default)]
pub enum Mode {
    #[default]
    Dev,
    Test,
    Prod,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
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
            genesis: None,
            mode: Mode::Dev,
            testnet: false,
            storage: StorageConfig::InMemory,
            db_path: None,
            db_options: None,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
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
