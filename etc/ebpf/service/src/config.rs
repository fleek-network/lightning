use std::path::{Path, PathBuf};
use std::sync::Arc;

use resolved_pathbuf::ResolvedPathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::map::{FileOpenRule, PacketFilterRule};

const ROOT_CONFIG_DIR: &str = "~/.lightning/ebpf/config";
const PACKET_FILTER_PATH: &str = "filters.json";
const PROFILES_PATH: &str = "profiles.json";

/// Configuration source.
///
/// Utility object for reading/writting to configuration files.
#[derive(Clone, Default)]
pub struct ConfigSource {
    paths: Arc<PathConfig>,
}

impl ConfigSource {
    pub fn new() -> anyhow::Result<Self> {
        let result = Self::default();

        std::fs::create_dir_all(&result.paths.root_path)?;

        let mut tmp = PathBuf::new();
        tmp.push("tmp");
        tmp.push(&result.paths.root_path.as_path());
        std::fs::create_dir_all(tmp)?;

        Ok(result)
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
        tmp_path.push(self.paths.packet_filters_path.as_path());

        let mut tmp = fs::File::create(tmp_path.as_path()).await?;
        let bytes = serde_json::to_string(&filters)?;
        tmp.write_all(bytes.as_bytes()).await?;
        tmp.sync_all().await?;

        let mut dst = PathBuf::new();
        dst.push(self.paths.root_path.as_path());
        dst.push(self.paths.packet_filters_path.as_path());

        fs::rename(tmp_path, dst).await?;

        Ok(())
    }

    /// Read packet-filters from storage.
    pub async fn read_profiles(&self) -> anyhow::Result<Vec<FileOpenRule>> {
        let content = fs::read_to_string(&self.paths.profiles_path).await?;
        serde_json::from_str(&content).map_err(Into::into)
    }

    /// Writes packet-filters to storage.
    pub async fn write_profiles(&self, filters: Vec<FileOpenRule>) -> anyhow::Result<()> {
        let mut tmp_path = PathBuf::new();
        tmp_path.push(self.paths.root_path.as_path());
        tmp_path.push("tmp");
        tmp_path.push(self.paths.profiles_path.as_path());

        let mut tmp = fs::File::create(tmp_path.as_path()).await?;
        let bytes = serde_json::to_string(&filters)?;
        tmp.write_all(bytes.as_bytes()).await?;
        tmp.sync_all().await?;

        let mut dst = PathBuf::new();
        dst.push(self.paths.root_path.as_path());
        dst.push(self.paths.profiles_path.as_path());

        fs::rename(tmp_path, dst).await?;

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
            profiles_path: ResolvedPathBuf::try_from(
                format!("{ROOT_CONFIG_DIR}/{PROFILES_PATH}").as_str(),
            )
            .expect("Hardcoded path"),
        }
    }
}
