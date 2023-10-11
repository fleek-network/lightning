use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    /// Whether this node is activated as an archive node or not
    pub is_archive: bool,
    /// Path to the database used by the narwhal implementation.
    pub store_path: Option<ResolvedPathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            is_archive: false,
            store_path: None,
        }
    }
}
