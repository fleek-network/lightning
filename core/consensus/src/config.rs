use lightning_utils::config::LIGHTNING_HOME_DIR;
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ConsensusConfig {
    /// Path to the database used by the narwhal implementation.
    pub store_path: ResolvedPathBuf,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            store_path: LIGHTNING_HOME_DIR
                .join("data/narwhal_store")
                .try_into()
                .expect("Failed to resolve path"),
        }
    }
}
