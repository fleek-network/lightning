use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    root: ResolvedPathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            root: ResolvedPathBuf::try_from("~/.lightning/blockstore").unwrap(),
        }
    }
}
