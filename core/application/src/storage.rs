use std::path::PathBuf;

use atomo::{InMemoryStorage, StorageBackend, StorageBackendConstructor};
use atomo_rocks::{Options, RocksBackend, RocksBackendBuilder};

pub enum AtomoStorageBuilder<'a> {
    InMemory(InMemoryStorage),
    RocksDb(RocksBackendBuilder<'a>),
}

impl<'a> AtomoStorageBuilder<'a> {
    #[inline(always)]
    pub fn new<P: Into<PathBuf>>(path: Option<P>) -> Self {
        match path {
            Some(path) => AtomoStorageBuilder::RocksDb(RocksBackendBuilder::new(path)),
            None => AtomoStorageBuilder::InMemory(InMemoryStorage::default()),
        }
    }

    #[inline(always)]
    pub fn read_only(self) -> Self {
        match self {
            AtomoStorageBuilder::InMemory(builder) => AtomoStorageBuilder::InMemory(builder),
            AtomoStorageBuilder::RocksDb(builder) => {
                let builder = builder.read_only();
                AtomoStorageBuilder::RocksDb(builder)
            },
        }
    }

    #[inline(always)]
    pub fn with_options(self, opts: Options) -> Self {
        match self {
            AtomoStorageBuilder::InMemory(builder) => AtomoStorageBuilder::InMemory(builder),
            AtomoStorageBuilder::RocksDb(builder) => {
                let builder = builder.with_options(opts);
                AtomoStorageBuilder::RocksDb(builder)
            },
        }
    }

    #[inline(always)]
    #[allow(unused)]
    pub fn with_table_option(self, name: &str, opts: Options) -> Self {
        match self {
            AtomoStorageBuilder::InMemory(builder) => AtomoStorageBuilder::InMemory(builder),
            AtomoStorageBuilder::RocksDb(builder) => {
                let builder = builder.with_table_option(name, opts);
                AtomoStorageBuilder::RocksDb(builder)
            },
        }
    }

    #[inline(always)]
    #[allow(unused)]
    #[allow(clippy::wrong_self_convention)]
    pub fn from_checkpoint(self, hash: [u8; 32], checkpoint: &'a [u8]) -> Self {
        match self {
            AtomoStorageBuilder::InMemory(builder) => AtomoStorageBuilder::InMemory(builder),
            AtomoStorageBuilder::RocksDb(builder) => {
                let builder = builder.from_checkpoint(hash, checkpoint);
                AtomoStorageBuilder::RocksDb(builder)
            },
        }
    }
}

impl<'a> StorageBackendConstructor for AtomoStorageBuilder<'a> {
    type Storage = AtomoStorage;

    type Error = anyhow::Error;

    fn open_table(&mut self, name: String) {
        match self {
            AtomoStorageBuilder::InMemory(builder) => builder.open_table(name),
            AtomoStorageBuilder::RocksDb(builder) => builder.open_table(name),
        }
    }

    fn build(self) -> Result<Self::Storage, Self::Error> {
        match self {
            AtomoStorageBuilder::InMemory(builder) => {
                let storage = builder.build()?;
                Ok(AtomoStorage::InMemory(storage))
            },
            AtomoStorageBuilder::RocksDb(builder) => {
                let storage = builder.build()?;
                Ok(AtomoStorage::RocksDb(storage))
            },
        }
    }
}

pub enum AtomoStorage {
    InMemory(InMemoryStorage),
    RocksDb(RocksBackend),
}

impl From<InMemoryStorage> for AtomoStorage {
    fn from(storage: InMemoryStorage) -> Self {
        AtomoStorage::InMemory(storage)
    }
}

impl From<RocksBackend> for AtomoStorage {
    fn from(storage: RocksBackend) -> Self {
        AtomoStorage::RocksDb(storage)
    }
}

impl StorageBackend for AtomoStorage {
    fn commit(&self, batch: atomo::batch::VerticalBatch) {
        match &self {
            AtomoStorage::InMemory(storage) => storage.commit(batch),
            AtomoStorage::RocksDb(storage) => storage.commit(batch),
        }
    }

    fn keys(&self, tid: u8) -> Vec<atomo::batch::BoxedVec> {
        match &self {
            AtomoStorage::InMemory(storage) => storage.keys(tid),
            AtomoStorage::RocksDb(storage) => storage.keys(tid),
        }
    }

    fn get(&self, tid: u8, key: &[u8]) -> Option<Vec<u8>> {
        match &self {
            AtomoStorage::InMemory(storage) => storage.get(tid, key),
            AtomoStorage::RocksDb(storage) => storage.get(tid, key),
        }
    }

    fn contains(&self, tid: u8, key: &[u8]) -> bool {
        match &self {
            AtomoStorage::InMemory(storage) => storage.contains(tid, key),
            AtomoStorage::RocksDb(storage) => storage.contains(tid, key),
        }
    }

    fn serialize(&self) -> Option<Vec<u8>> {
        match &self {
            AtomoStorage::InMemory(_storage) => None,
            AtomoStorage::RocksDb(storage) => Some(storage.serialize()),
        }
    }
}
