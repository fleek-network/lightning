use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};

pub const ROOT_DIR_DEFAULT: &str = "~/.lightning/blockstore";
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
            root: ResolvedPathBuf::try_from(ROOT_DIR_DEFAULT).unwrap(),
        }
    }
}
