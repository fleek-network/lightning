use std::{
    fmt::Debug,
    path::{Path, PathBuf},
};

use derive_more::{AsRef, Deref};
use resolve_path::PathResolveExt;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// This type is a wrapper around a normal [`PathBuf`] that is resolved upon construction. This
/// happens through the [`resolve_path`] crate.
///
/// Additionally, this wrapper preserves the original path as well, and uses the original when
/// serialized using [`serde`]. This makes this wrapper a safe tool when dealing with user provided
/// path. For example either through command line arguments or configuration values.
///
/// Preserving the original allows us to avoid modifying the user configuration when serializing
/// the configuration back to the disk.
#[derive(Deref, AsRef)]
pub struct ResolvedPathBuf {
    #[as_ref]
    #[deref]
    resolved: PathBuf,
    /// If the resolved path is the same as the original we don't store a copy
    /// of the original. This can happen if the original path is already an
    /// absolute path.
    original: Option<PathBuf>,
}

impl TryFrom<PathBuf> for ResolvedPathBuf {
    type Error = std::io::Error;

    fn try_from(original: PathBuf) -> Result<Self, Self::Error> {
        let resolved = original.try_resolve()?.to_path_buf();
        let original = (resolved != original).then_some(original);
        Ok(Self { resolved, original })
    }
}

impl TryFrom<&str> for ResolvedPathBuf {
    type Error = std::io::Error;

    fn try_from(path: &str) -> Result<Self, Self::Error> {
        let original: PathBuf = path.into();
        let resolved = original.try_resolve()?.to_path_buf();
        let original = (resolved != original).then_some(original);
        Ok(Self { resolved, original })
    }
}

impl AsRef<Path> for ResolvedPathBuf {
    fn as_ref(&self) -> &Path {
        self.resolved.as_path()
    }
}

impl Debug for ResolvedPathBuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.resolved.fmt(f)
    }
}

impl Serialize for ResolvedPathBuf {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.original
            .as_ref()
            .unwrap_or(&self.resolved)
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ResolvedPathBuf {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let original: PathBuf = PathBuf::deserialize(deserializer)?;
        ResolvedPathBuf::try_from(original)
            .map_err(|e| serde::de::Error::custom(format!("Failed to resolve path: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_as_ref() {
        let p = ResolvedPathBuf::try_from(PathBuf::from(r"~/x")).unwrap();
        let x: &PathBuf = p.as_ref();
        assert_eq!(&p.resolved, x);
        assert!(x.is_absolute());
    }

    #[test]
    fn test_deref() {
        let p = ResolvedPathBuf::try_from(PathBuf::from(r"~/x")).unwrap();
        let x: &PathBuf = &p;
        assert_eq!(&p.resolved, x);
        assert!(x.is_absolute());
    }

    #[test]
    fn serde_serialize() {
        let o = PathBuf::from(r"~/x");
        let r = ResolvedPathBuf::try_from(o.clone()).unwrap();

        let o_json = serde_json::to_vec(&o).unwrap();
        let r_json = serde_json::to_vec(&r).unwrap();

        assert_eq!(o_json, r_json);
    }

    #[test]
    fn serde_deserialize() {
        let o = PathBuf::from(r"~/x");
        let o_json = serde_json::to_vec(&o).unwrap();
        let r: ResolvedPathBuf = serde_json::from_slice(&o_json).unwrap();
        assert_eq!(r.original, Some(o));
        assert!(r.is_absolute());
    }
}
