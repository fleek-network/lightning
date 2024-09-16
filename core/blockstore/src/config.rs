use lightning_utils::config::LIGHTNING_HOME_DIR;
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub root: ResolvedPathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            root: LIGHTNING_HOME_DIR
                .join("blockstore")
                .try_into()
                .expect("Failed to resolve path"),
        }
    }
}
