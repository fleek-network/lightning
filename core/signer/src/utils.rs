use std::{
    fs::{self, create_dir_all, File},
    io::Write,
    os::unix::fs::PermissionsExt,
    path::Path,
};

pub fn save<T: AsRef<[u8]>>(path: &Path, data: T) -> anyhow::Result<()> {
    // Mostly taken from: https://github.com/fleek-network/ursa/blob/feat/pod/crates/ursa/src/ursa/identity.rs
    create_dir_all(path.parent().unwrap())?;
    let mut file = File::create(path)?;
    file.write_all(data.as_ref())?;
    file.sync_all()?;
    let mut perms = file.metadata()?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)?;
    Ok(())
}
