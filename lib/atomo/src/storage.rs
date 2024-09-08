use std::fmt::Debug;
use std::sync::{Arc, Mutex};

use dashmap::DashMap;

use crate::batch::{BoxedVec, Operation, VerticalBatch};

pub trait StorageBackendConstructor {
    /// The storage API.
    type Storage: StorageBackend;

    /// The error that can be produced while opening the database.
    type Error: Debug;

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
    fn keys(&self, tid: u8) -> Box<dyn Iterator<Item = BoxedVec> + '_>;

    /// Get the value associated with the given key from the provided table.
    fn get(&self, tid: u8, key: &[u8]) -> Option<Vec<u8>>;

    /// Get all of the key/value pairs from the provided table.
    fn get_all(&self, tid: u8) -> Box<dyn Iterator<Item = (BoxedVec, BoxedVec)> + '_>;

    /// Returns true if the table contains the provided key.
    fn contains(&self, tid: u8, key: &[u8]) -> bool;

    /// Serialize the backend to a series of bytes.
    fn serialize(&self, _exclude_tables: &[String]) -> Option<Vec<u8>> {
        None
    }
}

/// The in memory storage backend.
///
/// This is useful for testing and for simple use cases.
///
/// It's shared across threads and can be cloned freely. Cloning will NOT duplicate the underlying
/// data but rather create a new handle to the same underlying data, since the data is reference
/// counted and using a mutex.
#[derive(Default, Clone)]
pub struct InMemoryStorage(Arc<Mutex<Vec<DashMap<BoxedVec, BoxedVec, fxhash::FxBuildHasher>>>>);

// For the in memory database the constructor can be as same as the actual object.
impl StorageBackendConstructor for InMemoryStorage {
    type Storage = Self;
    type Error = std::convert::Infallible;

    #[inline]
    fn open_table(&mut self, _name: String) {
        self.0.lock().unwrap().push(DashMap::default())
    }

    fn build(self) -> Result<Self::Storage, Self::Error> {
        Ok(self)
    }
}

impl StorageBackend for InMemoryStorage {
    #[inline]
    fn commit(&self, batch: VerticalBatch) {
        for (table, batch) in self
            .0
            .lock()
            .unwrap()
            .iter()
            .zip(batch.into_raw().into_iter())
        {
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
    fn keys(&self, tid: u8) -> Box<dyn Iterator<Item = BoxedVec> + '_> {
        let mut collection = Vec::new();
        for item in self.0.lock().unwrap()[tid as usize].iter() {
            collection.push(item.key().clone());
        }
        Box::new(collection.into_iter())
    }

    #[inline]
    fn get(&self, tid: u8, key: &[u8]) -> Option<Vec<u8>> {
        self.0.lock().unwrap()[tid as usize]
            .get(key)
            .map(|v| v.to_vec())
    }

    #[inline]
    fn get_all(&self, tid: u8) -> Box<dyn Iterator<Item = (BoxedVec, BoxedVec)> + '_> {
        Box::new(
            self.0.lock().unwrap()[tid as usize]
                .iter()
                .map(|item| (item.key().clone(), item.value().clone()))
                .collect::<Vec<_>>()
                .into_iter(),
        )
    }

    #[inline]
    fn contains(&self, tid: u8, key: &[u8]) -> bool {
        self.0.lock().unwrap()[tid as usize].contains_key(key)
    }
}
