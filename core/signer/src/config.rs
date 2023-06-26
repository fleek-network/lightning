use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    keystore_path: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            keystore_path: "~/.draco/keystore".into(),
        }
    }
}
