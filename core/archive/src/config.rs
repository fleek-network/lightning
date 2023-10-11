use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    /// Whether this node is activated as an archive node or not
    pub is_archive: bool,
    /// Path to the database used by the narwhal implementation.
    pub store_path: Option<ResolvedPathBuf>,
}
