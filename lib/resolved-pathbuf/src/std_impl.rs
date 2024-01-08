use std::borrow::Borrow;
use std::fmt::Debug;
use std::hash::Hash;
use std::path::{Path, PathBuf};

use crate::ResolvedPathBuf;

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

impl Hash for ResolvedPathBuf {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.resolved.hash(state)
    }
}

impl Eq for ResolvedPathBuf {}

impl PartialEq for ResolvedPathBuf {
    fn eq(&self, other: &Self) -> bool {
        self.resolved.eq(&other.resolved)
    }
}

impl Ord for ResolvedPathBuf {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.resolved.cmp(&other.resolved)
    }
}

impl PartialOrd for ResolvedPathBuf {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Borrow<PathBuf> for ResolvedPathBuf {
    fn borrow(&self) -> &PathBuf {
        &self.resolved
    }
}
