use std::time::Duration;

use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub submit_interval: Duration,
    /// Path to the database where the dacks are stored.
    pub db_path: ResolvedPathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            submit_interval: Duration::from_secs(10),
            db_path: "~/.lightning/data/dack_aggregator"
                .try_into()
                .expect("Failed to resolve path"),
        }
    }
}
