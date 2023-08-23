use std::error::Error;

use dashmap::DashMap;

use crate::batch::{BoxedVec, Operation, VerticalBatch};

pub trait StorageBackendConstructor {
    /// The storage API.
    type Storage: StorageBackend;

    /// The error that can be produced while opening the database.
    type Error: Error;

    /// Called during the initialization
    fn open_table(&mut self, name: String);

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
    fn keys(&self, tid: u8) -> Vec<BoxedVec>;

    /// Get the value associated with the given key from the provided table.
    fn get(&self, tid: u8, key: &[u8]) -> Option<Vec<u8>>;

    /// Returns true if the table contains the provided key.
    fn contains(&self, tid: u8, key: &[u8]) -> bool;
}

#[derive(Default)]
pub struct InMemoryStorage(Vec<DashMap<BoxedVec, BoxedVec, fxhash::FxBuildHasher>>);

// For the in memory database the constructor can be as same as the actual object.
impl StorageBackendConstructor for InMemoryStorage {
    type Storage = Self;
    type Error = std::convert::Infallible;

    #[inline]
    fn open_table(&mut self, _name: String) {
        self.0.push(DashMap::default())
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

    fn keys(&self, tid: u8) -> Vec<BoxedVec> {
        let mut collection = Vec::new();
        for item in self.0[tid as usize].iter() {
            collection.push(item.key().clone());
        }
        collection
    }

    #[inline]
    fn get(&self, tid: u8, key: &[u8]) -> Option<Vec<u8>> {
        self.0[tid as usize].get(key).map(|v| v.to_vec())
    }

    #[inline]
    fn contains(&self, tid: u8, key: &[u8]) -> bool {
        self.0[tid as usize].contains_key(key)
    }
}
