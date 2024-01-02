use std::collections::BTreeMap;

use smol_str::SmolStr;
use thiserror::Error;

use super::{Directory, Link};
use crate::directory::{hash_directory, DirectoryEntry};

/// A simple incremental directory builder.
pub struct DirectoryBuilder {
    entries: BTreeMap<SmolStr, Link>,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum DirectoryBuilderError {
    #[error("Entry names must be unique.")]
    NameAlreadyInUse,
}

impl DirectoryBuilder {
    pub fn insert(
        &mut self,
        name: impl Into<SmolStr>,
        link: Link,
    ) -> Result<(), DirectoryBuilderError> {
        let name = name.into();
        self.insert_(name, link)
    }

    fn insert_(&mut self, name: SmolStr, link: Link) -> Result<(), DirectoryBuilderError> {
        // TODO(qti3e): Replace with `try_insert` once it's stable in std.
        match self.entries.entry(name) {
            std::collections::btree_map::Entry::Vacant(e) => {
                e.insert(link);
                Ok(())
            },
            std::collections::btree_map::Entry::Occupied(_) => {
                Err(DirectoryBuilderError::NameAlreadyInUse)
            },
        }
    }

    pub fn build(self) -> Directory {
        let entries: Vec<DirectoryEntry> = self
            .entries
            .into_iter()
            .map(|(name, link)| DirectoryEntry::new(name, link))
            .collect();

        let tree = hash_directory(true, &entries).tree.unwrap();

        Directory { entries, tree }
    }
}
