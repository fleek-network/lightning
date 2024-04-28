use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use resolved_pathbuf::ResolvedPathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::map::{PacketFilterRule, Profile};

const ROOT_CONFIG_DIR: &str = "~/.lightning/ebpf/config";
const PACKET_FILTER_PATH: &str = "filters.json";
const PROFILES_PATH: &str = "~/.lightning/ebpf/profiles";

/// Configuration source.
///
/// Utility object for reading/writting to configuration files.
#[derive(Clone, Debug)]
pub struct ConfigSource {
    paths: Arc<PathConfig>,
}

impl ConfigSource {
    pub fn create_config() -> anyhow::Result<Self> {
        let config = PathConfig::default();

        std::fs::create_dir_all(&config.root_path)?;

        let mut tmp = PathBuf::new();
        tmp.push(&config.root_path.as_path());
        tmp.push("tmp");
        std::fs::create_dir_all(tmp)?;

        std::fs::create_dir_all(config.profiles_path.as_path())?;

        // Create file for packet filters.
        File::create(config.packet_filters_path.as_path())?;

        Ok(Self {
            paths: Arc::new(config)
        })
    }

    pub fn new() -> Self  {
        Self {
            paths: Arc::new(PathConfig::default())
        }
    }

    pub fn packet_filers_path(&self) -> &Path {
        self.paths.packet_filters_path.as_path()
    }

    pub fn profiles_path(&self) -> &Path {
        self.paths.profiles_path.as_path()
    }

    /// Reads packet-filters from storage.
    pub async fn read_packet_filters(&self) -> anyhow::Result<Vec<PacketFilterRule>> {
        let content = fs::read_to_string(&self.paths.packet_filters_path).await?;
        serde_json::from_str(&content).map_err(Into::into)
    }

    /// Writes packet-filters to storage.
    pub async fn write_packet_filters(&self, filters: Vec<PacketFilterRule>) -> anyhow::Result<()> {
        let mut tmp_path = PathBuf::new();
        tmp_path.push(self.paths.root_path.as_path());
        tmp_path.push("tmp");
        tmp_path.push(PACKET_FILTER_PATH);

        let mut tmp = fs::File::create(tmp_path.as_path()).await?;
        let bytes = serde_json::to_string(&filters)?;
        tmp.write_all(bytes.as_bytes()).await?;
        tmp.sync_all().await?;

        let mut dst = PathBuf::new();
        dst.push(self.paths.packet_filters_path.as_path());
        fs::rename(tmp_path, dst).await?;

        Ok(())
    }

    /// Get names of profiles.
    pub async fn get_profiles(&self) -> anyhow::Result<Vec<Profile>> {
        let mut result = Vec::new();
        let mut files = fs::read_dir(&self.paths.profiles_path).await?;
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
                path.push(self.paths.profiles_path.as_path());
                path.push(name);
                std::fs::read_to_string(path.as_path())?
            },
            None => {
                let mut path = PathBuf::new();
                path.push(self.paths.profiles_path.as_path());
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
                path.push(self.paths.profiles_path.as_path());
                path.push(name);
                fs::read_to_string(path.as_path()).await?
            },
            None => {
                let mut path = PathBuf::new();
                path.push(self.paths.profiles_path.as_path());
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
            dst.push(self.paths.root_path.as_path());
            dst.push(self.paths.profiles_path.as_path());
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
            tmp_path.push(self.paths.root_path.as_path());
            tmp_path.push("tmp");
            tmp_path.push(self.paths.profiles_path.as_path());
            tmp_path.push(fname.clone());

            let mut tmp = fs::File::create(tmp_path.as_path()).await?;
            let bytes = serde_json::to_string(&profile)?;
            tmp.write_all(bytes.as_bytes()).await?;
            tmp.sync_all().await?;

            let mut dst = PathBuf::new();
            dst.push(self.paths.root_path.as_path());
            dst.push(self.paths.profiles_path.as_path());
            dst.push(fname);

            fs::rename(tmp_path, dst).await?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct PathConfig {
    pub root_path: ResolvedPathBuf,
    pub packet_filters_path: ResolvedPathBuf,
    pub profiles_path: ResolvedPathBuf,
}

impl Default for PathConfig {
    fn default() -> Self {
        Self {
            root_path: ResolvedPathBuf::try_from(ROOT_CONFIG_DIR).expect("Hardcoded path"),
            packet_filters_path: ResolvedPathBuf::try_from(
                format!("{ROOT_CONFIG_DIR}/{PACKET_FILTER_PATH}").as_str(),
            )
            .expect("Hardcoded path"),
            profiles_path: ResolvedPathBuf::try_from(PROFILES_PATH).expect("Hardcoded path"),
        }
    }
}
