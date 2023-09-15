use std::fs::create_dir_all;

use anyhow::{bail, Result};
use resolved_pathbuf::ResolvedPathBuf;

pub fn ensure_parent_exist(path: &ResolvedPathBuf) -> Result<()> {
    if let Some(parent_dir) = path.parent() {
        Ok(create_dir_all(parent_dir)?)
    } else {
        bail!("Failed to get parent directory from given file: {:?}", path)
    }
}
