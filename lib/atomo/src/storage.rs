use std::fmt::Debug;

use dashmap::DashMap;

use crate::batch::{BoxedVec, Operation, VerticalBatch};

pub type TableIndex = u8;

pub trait StorageBackendConstructor {
    /// The storage API.
    type Storage: StorageBackend + Send + Sync;

    /// The error that can be produced while opening the database.
    type Error: Debug;

    /// Called during the initialization.
    /// Returns the table index.
    fn open_table(&mut self, name: String) -> TableIndex;

    /// Build the storage object and return it. If there is any error
    /// should return the error.
    fn build(self) -> Result<Self::Storage, Self::Error>;
}

/// The persistence backend in Atomo provides the binding layer with any persistence layer
/// of you choice and allows custom implementations of the underlying data storage.
pub trait StorageBackend {
    /// Write the changes to the disk.
    fn commit(&self, batch: VerticalBatch);

    /// Return all of the keys from a table.
    fn keys(&self, tid: TableIndex) -> Vec<BoxedVec>;

    /// Get the value associated with the given key from the provided table.
    fn get(&self, tid: TableIndex, key: &[u8]) -> Option<Vec<u8>>;

    /// Returns true if the table contains the provided key.
    fn contains(&self, tid: TableIndex, key: &[u8]) -> bool;

    /// Serialize the backend to a series of bytes.
    fn serialize(&self) -> Option<Vec<u8>> {
        None
    }
}

#[derive(Default, Clone)]
pub struct InMemoryStorage(Vec<DashMap<BoxedVec, BoxedVec, fxhash::FxBuildHasher>>);

// For the in memory database the constructor can be as same as the actual object.
impl StorageBackendConstructor for InMemoryStorage {
    type Storage = Self;
    type Error = std::convert::Infallible;

    #[inline]
    fn open_table(&mut self, _name: String) -> TableIndex {
        self.0.push(DashMap::default());
        (self.0.len() - 1).try_into().unwrap()
    }

    fn build(self) -> Result<Self::Storage, Self::Error> {
        Ok(self)
    }
}

impl StorageBackend for InMemoryStorage {
    #[inline]
    fn commit(&self, batch: VerticalBatch) {
        for (table, batch) in self.0.iter().zip(batch.into_raw().into_iter()) {
            for (key, operation) in batch.into_iter() {
                match operation {
                    Operation::Remove => {
                        table.remove(&key);
                    },
                    Operation::Insert(value) => {
                        table.insert(key, value);
                    },
                }
            }
        }
    }

    #[inline]
    fn keys(&self, tid: TableIndex) -> Vec<BoxedVec> {
        let mut collection = Vec::new();
        for item in self.0[tid as usize].iter() {
            collection.push(item.key().clone());
        }
        collection
    }

    #[inline]
    fn get(&self, tid: TableIndex, key: &[u8]) -> Option<Vec<u8>> {
        self.0[tid as usize].get(key).map(|v| v.to_vec())
    }

    #[inline]
    fn contains(&self, tid: TableIndex, key: &[u8]) -> bool {
        self.0[tid as usize].contains_key(key)
    }
}
