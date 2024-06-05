use lightning_utils::config::LIGHTNING_HOME_DIR;
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};

pub const INTERNAL_DIR: &str = "internal";
pub const BLOCK_DIR: &str = "block";
pub const TMP_DIR: &str = "tmp";

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
