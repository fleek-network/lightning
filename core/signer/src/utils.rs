use std::{
    fs::{self, create_dir_all, File},
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use anyhow::anyhow;

pub fn save<T: AsRef<[u8]>>(path: &Path, data: T) -> anyhow::Result<()> {
    // Mostly taken from: https://github.com/fleek-network/ursa/blob/feat/pod/crates/ursa/src/ursa/identity.rs
    let path = expand_path(path)?;
    create_dir_all(path.parent().unwrap())?;
    let mut file = File::create(path.as_path())?;
    file.write_all(data.as_ref())?;
    file.sync_all()?;
    let mut perms = file.metadata()?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)?;
    Ok(())
}

fn expand_path(path: &Path) -> anyhow::Result<PathBuf> {
    if path.starts_with("~") {
        let path = path.strip_prefix("~")?;
        let home_dir = dirs::home_dir().ok_or(anyhow!("Failed to obtain home directory."))?;
        let full_path = home_dir.join(path);
        Ok(full_path)
    } else {
        Ok(path.to_owned())
    }
}
