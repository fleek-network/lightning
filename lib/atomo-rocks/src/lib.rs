//! A [`rocksdb`] storage backend implementation for [`atomo`].

mod serialization;
use std::fs::{self};
use std::path::PathBuf;

use anyhow::Result;
use atomo::batch::Operation;
use atomo::{AtomoBuilder, DefaultSerdeBackend, StorageBackend, StorageBackendConstructor};
use fxhash::FxHashMap;
/// Re-export of [`rocksdb::Options`].
pub use rocksdb::Options;
pub use rocksdb::{Cache, Env, DB};
use rocksdb::{ColumnFamilyDescriptor, WriteBatch};
pub use serialization::{build_db_from_checkpoint, serialize_db};

/// Helper alias for an [`atomo::AtomoBuilder`] using a [`RocksBackendBuilder`].
pub type AtomoBuilderWithRocks<S = DefaultSerdeBackend> = AtomoBuilder<RocksBackendBuilder, S>;

/// Builder for a new [`rocksdb::DB`] backend.
///
/// # Example
///
/// ```
/// use atomo::DefaultSerdeBackend;
/// use atomo_rocks::{AtomoBuilderWithRocks, Options, RocksBackendBuilder};
///
/// let path = "example-rocksdb";
/// let mut options = Options::default();
/// options.create_if_missing(true);
/// options.create_missing_column_families(true);
/// let rocksdb = RocksBackendBuilder::new(path).with_options(options);
///
/// let atomo = AtomoBuilderWithRocks::<DefaultSerdeBackend>::new(rocksdb)
///     .with_table::<u64, u64>("example")
///     .build()
///     .unwrap();
/// let table_res = atomo.resolve::<u64, u64>("example");
///
/// // cleanup
/// drop(atomo);
/// std::fs::remove_dir_all(path).unwrap();
/// ```
pub struct RocksBackendBuilder {
    path: PathBuf,
    options: Options,
    columns: Vec<String>,
    column_options: FxHashMap<String, Options>,
    checkpoint: Option<([u8; 32], Vec<u8>)>,
}

impl RocksBackendBuilder {
    /// Create a new builder at the given path.
    #[inline(always)]
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        Self {
            path: path.into(),
            options: Default::default(),
            columns: Default::default(),
            column_options: Default::default(),
            checkpoint: Default::default(),
        }
    }

    /// Provide an [`Options`] object for the overall database.
    #[inline(always)]
    pub fn with_options(mut self, opts: Options) -> Self {
        self.options = opts;
        self
    }

    /// Provide an [`Options`] object for a specific table to the builder. All missing table options
    /// are always set to the default.
    #[inline(always)]
    pub fn with_table_option(mut self, name: &str, opts: Options) -> Self {
        self.column_options.insert(name.into(), opts);
        self
    }

    /// Provide a checkpoint from which the database will be built.
    /// Warning: providing a checkpoint will overwrite the existing database at the specified path,
    /// if there is one.
    #[inline(always)]
    pub fn from_checkpoint(mut self, hash: [u8; 32], checkpoint: Vec<u8>) -> Self {
        self.checkpoint = Some((hash, checkpoint));
        self
    }
}

impl StorageBackendConstructor for RocksBackendBuilder {
    type Storage = RocksBackend;

    type Error = anyhow::Error;

    fn open_table(&mut self, name: String) {
        self.columns.push(name)
    }

    fn build(mut self) -> Result<Self::Storage, Self::Error> {
        let cf_iter: Vec<_> = self
            .columns
            .iter()
            .map(|name| {
                ColumnFamilyDescriptor::new(
                    name,
                    self.column_options.remove(name).unwrap_or_default(),
                )
            })
            .collect();
        let db = match self.checkpoint {
            Some((hash, checkpoint)) => {
                // We try to build the db from a checkpoint in a temporary dir.
                let mut tmp_path = self.path.clone();
                tmp_path.pop();
                tmp_path.push("tmp");
                if tmp_path.exists() {
                    fs::remove_dir_all(&tmp_path)?;
                }
                fs::create_dir_all(&tmp_path)?;
                let (_db, column_names) =
                    build_db_from_checkpoint(&tmp_path, hash, &checkpoint, self.options.clone())?;
                // If the build was successful, we move the db over to the actual directory.
                if self.path.exists() {
                    fs::remove_dir_all(&self.path)?;
                }
                fs::rename(&tmp_path, &self.path)?;
                if tmp_path.exists() {
                    fs::remove_dir_all(&tmp_path)?;
                }
                let cf_iter: Vec<_> = column_names
                    .iter()
                    .map(|name| {
                        ColumnFamilyDescriptor::new(
                            name,
                            self.column_options.remove(name).unwrap_or_default(),
                        )
                    })
                    .collect();
                let mut options = self.options;
                // The database should exist at this point.
                options.create_if_missing(false);
                DB::open_cf_descriptors(&options, &self.path, cf_iter)?
            },
            None => DB::open_cf_descriptors(&self.options, self.path, cf_iter)?,
        };

        Ok(RocksBackend {
            columns: self.columns,
            db,
        })
    }
}

/// RocksDB persistence backend for [`atomo`].
pub struct RocksBackend {
    db: rocksdb::DB,
    columns: Vec<String>,
}

impl RocksBackend {
    pub fn serialize(&self) -> Vec<u8> {
        // We can safely unwrap here.
        // This will only panic if the table names in `columns` are not consistent with the
        // database.
        serialize_db(&self.db, &self.columns).unwrap()
    }
}

impl StorageBackend for RocksBackend {
    fn commit(&self, batch: atomo::batch::VerticalBatch) {
        let mut inner_batch = WriteBatch::default();
        for (table, batch) in self.columns.iter().zip(batch.into_raw().into_iter()) {
            let cf = self.db.cf_handle(table).unwrap();
            for (key, operation) in batch {
                match operation {
                    Operation::Insert(value) => {
                        inner_batch.put_cf(&cf, key, value);
                    },
                    Operation::Remove => {
                        inner_batch.delete_cf(&cf, key);
                    },
                }
            }
        }
        self.db
            .write(inner_batch)
            .expect("failed to commit batch to rocksdb");
    }

    fn keys(&self, tid: u8) -> Vec<atomo::batch::BoxedVec> {
        let cf = self.db.cf_handle(&self.columns[tid as usize]).unwrap();
        self.db
            .iterator_cf(&cf, rocksdb::IteratorMode::Start)
            .map(|res| {
                res.expect("failed to get entry from column family iterator")
                    .0
            })
            .collect()
    }

    fn get(&self, tid: u8, key: &[u8]) -> Option<Vec<u8>> {
        let cf = self.db.cf_handle(&self.columns[tid as usize]).unwrap();
        self.db
            .get_cf(&cf, key)
            .expect("failed to get value from rocksdb")
    }

    fn contains(&self, tid: u8, key: &[u8]) -> bool {
        let cf = self.db.cf_handle(&self.columns[tid as usize]).unwrap();
        if self.db.key_may_exist_cf(&cf, key) {
            self.db
                .get_cf(&cf, key)
                .expect("failed to get value from rocksdb")
                .is_some()
        } else {
            false
        }
    }
}

/// Helper method to determine if there is currently a lock on your RocksDB
/// Will return true if there is any fs problems trying to access the the lock file(Like user
/// permission errors) but if the directory or lock file does not exist it will return false
/// indicating you are safe to open a db here.
#[cfg(not(target_os = "macos"))]
pub fn is_db_locked(mut path: PathBuf) -> bool {
    path.push("LOCK");

    if !path.exists() {
        return false;
    }

    if let Ok(file) = std::fs::OpenOptions::new().read(true).open(path) {
        fcntl::is_file_locked(&file, None).unwrap_or(true)
    } else {
        true
    }
}
#[cfg(target_os = "macos")]
pub fn is_db_locked(_path: PathBuf) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use rocksdb::Options;

    use crate::{AtomoBuilderWithRocks, RocksBackend, RocksBackendBuilder};

    const TEST_PATH: &str = "test-rocksdb";

    #[test]
    fn create_insert_and_query() {
        let path: PathBuf = TEST_PATH.parse().unwrap();
        if path.exists() {
            std::fs::remove_dir_all(path.clone()).expect("failed to remove old rocksdb");
        }

        // setup rocksdb
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);
        let rocksdb = RocksBackendBuilder::new(path.clone()).with_options(options);

        // setup atomo db
        let mut db = AtomoBuilderWithRocks::new(rocksdb)
            .with_table::<u64, u64>("test")
            .build()
            .unwrap();

        let query_runner = db.query();
        let table_res = db.resolve::<u64, u64>("test");

        // insert something to the table
        db.run(
            |ctx: &mut atomo::TableSelector<RocksBackend, atomo::BincodeSerde>| {
                let mut table_ref = table_res.get(ctx);
                table_ref.insert(0, 17);
            },
        );

        let barrier_1 = Arc::new(std::sync::Barrier::new(2));
        let barrier_2 = Arc::new(std::sync::Barrier::new(2));

        let c_1 = barrier_1.clone();
        let c_2 = barrier_2.clone();
        let handle = std::thread::spawn(move || {
            query_runner.run(|ctx: _| {
                let table_ref = table_res.get(ctx);
                assert_eq!(table_ref.get(0), Some(17));
                // Allow the main thread to continue.
                c_1.wait();
                // Waiting until the main thread updates the data.
                c_2.wait();
                assert_eq!(table_ref.get(0), Some(17))
            });

            // Run a second query this should get the new data.
            query_runner.run(|ctx: _| {
                let table_ref = table_res.get(ctx);
                assert_eq!(table_ref.get(0), Some(12));
            });
        });

        // Wait for the query thread to 'start' running the query.
        barrier_1.wait();

        // start the update
        db.run(|ctx: _| {
            let mut table_ref = table_res.get(ctx);
            table_ref.insert(0, 12);
        });

        // Allow the query thread to continue.
        barrier_2.wait();
        // Wait for the query thread to finish executing.
        let _ = handle.join();

        // cleanup
        std::fs::remove_dir_all(path).expect("failed to remove old rocksdb");
    }
}
