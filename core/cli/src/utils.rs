use std::fs::create_dir_all;
use std::path::PathBuf;

use anyhow::{bail, Result};

pub fn ensure_parent_exist(path: &PathBuf) -> Result<()> {
    if let Some(parent_dir) = path.parent() {
        create_dir_all(parent_dir)?;
        Ok(())
    } else {
        bail!("Failed to get parent directory from given file: {:?}", path)
    }
}
