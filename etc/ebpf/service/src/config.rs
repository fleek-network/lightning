use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use resolved_pathbuf::ResolvedPathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::map::{PacketFilterRule, Profile};

const TMP_DIR: &str = "~/.lightning/ebpf/tmp";
const PROFILES_DIR: &str = "~/.lightning/ebpf/profiles";
const PACKET_FILTER_CONFIG: &str = "~/.lightning/ebpf/filters.json";

/// Configuration source.
///
/// Utility object for reading/writting to configuration files.
#[derive(Clone, Debug)]
pub struct ConfigSource {
    paths: Arc<PathConfig>,
}

impl ConfigSource {
    pub fn new(config: PathConfig) -> Self {
        Self {
            paths: Arc::new(config),
        }
    }

    pub fn packet_filers_path(&self) -> &Path {
        self.paths.packet_filter.as_path()
    }

    pub fn profiles_path(&self) -> &Path {
        self.paths.profiles_dir.as_path()
    }

    /// Reads packet-filters from storage.
    pub async fn read_packet_filters(&self) -> anyhow::Result<Vec<PacketFilterRule>> {
        let content = fs::read_to_string(&self.paths.packet_filter).await?;
        serde_json::from_str(&content).map_err(Into::into)
    }

    /// Writes packet-filters to storage.
    pub async fn write_packet_filters(&self, filters: Vec<PacketFilterRule>) -> anyhow::Result<()> {
        let mut tmp_path = PathBuf::new();
        tmp_path.push(self.paths.tmp_dir.as_path());
        tmp_path.push("filters.json");

        let mut tmp = fs::File::create(tmp_path.as_path()).await?;
        let bytes = serde_json::to_string(&filters)?;
        tmp.write_all(bytes.as_bytes()).await?;
        tmp.sync_all().await?;

        let mut dst = PathBuf::new();
        dst.push(self.paths.packet_filter.as_path());
        fs::rename(tmp_path, dst).await?;

        Ok(())
    }

    /// Get names of profiles.
    pub async fn get_profiles(&self) -> anyhow::Result<Vec<Profile>> {
        let mut result = Vec::new();
        let mut files = fs::read_dir(&self.paths.profiles_dir).await?;
        while let Some(entry) = files.next_entry().await? {
            let ty = entry.file_type().await?;
            if ty.is_file() {
                let profile = self.read_profile(Some(&entry.file_name())).await?;
                result.push(profile);
            }
        }
        Ok(result)
    }

    pub fn blocking_read_profile(&self, name: Option<&OsStr>) -> anyhow::Result<Profile> {
        let content = match name {
            Some(name) => {
                let mut path = PathBuf::new();
                path.push(self.paths.profiles_dir.as_path());
                path.push(name);
                std::fs::read_to_string(path.as_path())?
            },
            None => {
                let mut path = PathBuf::new();
                path.push(self.paths.profiles_dir.as_path());
                path.push("global.json");
                std::fs::read_to_string(path)?
            },
        };
        serde_json::from_str(&content).map_err(Into::into)
    }

    /// Read profile.
    pub async fn read_profile(&self, name: Option<&OsStr>) -> anyhow::Result<Profile> {
        let content = match name {
            Some(name) => {
                let mut path = PathBuf::new();
                path.push(self.paths.profiles_dir.as_path());
                path.push(name);
                fs::read_to_string(path.as_path()).await?
            },
            None => {
                let mut path = PathBuf::new();
                path.push(self.paths.profiles_dir.as_path());
                path.push("global");
                fs::read_to_string(path).await?
            },
        };
        serde_json::from_str(&content).map_err(Into::into)
    }

    pub async fn delete_profiles(&self, profiles: HashSet<Option<PathBuf>>) -> anyhow::Result<()> {
        for profile in profiles {
            let fname = profile.unwrap_or("global".into());
            let mut dst = PathBuf::new();
            dst.push(self.paths.profiles_dir.as_path());
            dst.push(fname);

            fs::remove_file(dst).await.unwrap();
        }
        Ok(())
    }

    /// Writes packet-filters to storage.
    pub async fn write_profiles(&self, profiles: Vec<Profile>) -> anyhow::Result<()> {
        for profile in profiles {
            let fname = profile
                .name
                .as_ref()
                .map(|path| path.file_stem().unwrap().to_os_string())
                .unwrap_or("global".into());
            let mut tmp_path = PathBuf::new();
            tmp_path.push(self.paths.tmp_dir.as_path());
            tmp_path.push(fname.clone());

            let mut tmp = fs::File::create(tmp_path.as_path()).await?;
            let bytes = serde_json::to_string(&profile)?;
            tmp.write_all(bytes.as_bytes()).await?;
            tmp.sync_all().await?;

            let mut dst = PathBuf::new();
            dst.push(self.paths.profiles_dir.as_path());
            dst.push(fname);

            fs::rename(tmp_path, dst).await?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PathConfig {
    pub tmp_dir: PathBuf,
    pub packet_filter: PathBuf,
    pub profiles_dir: PathBuf,
}

impl Default for PathConfig {
    fn default() -> Self {
        Self {
            tmp_dir: ResolvedPathBuf::try_from(TMP_DIR)
                .expect("Hardcoded path")
                .to_path_buf(),
            packet_filter: ResolvedPathBuf::try_from(PACKET_FILTER_CONFIG)
                .expect("Hardcoded path")
                .to_path_buf(),
            profiles_dir: ResolvedPathBuf::try_from(PROFILES_DIR)
                .expect("Hardcoded path")
                .to_path_buf(),
        }
    }
}
