use std::path::Path;

use lightning_utils::config::LIGHTNING_HOME_DIR;
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};

const DEFAULT_RELATIVE_DATABASE_PATH: &str = "data/committee-beacon";

/// The committee beacon configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommitteeBeaconConfig {
    pub database: CommitteeBeaconDatabaseConfig,
}

impl CommitteeBeaconConfig {
    pub fn with_home_dir(self, home_dir: &Path) -> Self {
        let mut config = self.clone();
        config.database.path = home_dir
            .join(DEFAULT_RELATIVE_DATABASE_PATH)
            .try_into()
            .expect("Failed to resolve path");
        config
    }

    pub fn default_with_home_dir(home_dir: &Path) -> Self {
        Self::default().with_home_dir(home_dir)
    }
}

/// The committee beacon database configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitteeBeaconDatabaseConfig {
    pub path: ResolvedPathBuf,
}

impl Default for CommitteeBeaconDatabaseConfig {
    fn default() -> Self {
        Self {
            path: LIGHTNING_HOME_DIR
                .join(DEFAULT_RELATIVE_DATABASE_PATH)
                .try_into()
                .expect("Failed to resolve path"),
        }
    }
}
