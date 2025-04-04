//! A [`rocksdb`] storage backend implementation for [`atomo`].

mod serialization;
use std::collections::HashSet;
use std::fs::{self};
use std::path::PathBuf;

use anyhow::Result;
use atomo::batch::{BoxedVec, Operation};
use atomo::{AtomoBuilder, DefaultSerdeBackend, StorageBackend, StorageBackendConstructor};
use fxhash::FxHashMap;
/// Re-export of [`rocksdb::Options`].
pub use rocksdb::Options;
pub use rocksdb::{Cache, Env, DB};
use rocksdb::{ColumnFamilyDescriptor, WriteBatch};
pub use serialization::{build_db_from_checkpoint, serialize_db};

/// Helper alias for an [`atomo::AtomoBuilder`] using a [`RocksBackendBuilder`].
pub type AtomoBuilderWithRocks<'a, S = DefaultSerdeBackend> =
    AtomoBuilder<RocksBackendBuilder<'a>, S>;

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
pub struct RocksBackendBuilder<'a> {
    path: PathBuf,
    options: Options,
    columns: Vec<String>,
    column_options: FxHashMap<String, Options>,
    checkpoint: Option<([u8; 32], &'a [u8], &'a [String])>,
    read_only: bool,
}

impl<'a> RocksBackendBuilder<'a> {
    /// Create a new builder at the given path.
    #[inline(always)]
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        Self {
            path: path.into(),
            options: Default::default(),
            columns: Default::default(),
            column_options: Default::default(),
            checkpoint: Default::default(),
            read_only: false,
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
    pub fn from_checkpoint(
        mut self,
        hash: [u8; 32],
        checkpoint: &'a [u8],
        extra_tables: &'a [String],
    ) -> Self {
        self.checkpoint = Some((hash, checkpoint, extra_tables));
        self
    }

    /// Set the database to read-only mode.
    #[inline(always)]
    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }
}

impl StorageBackendConstructor for RocksBackendBuilder<'_> {
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
            Some((hash, checkpoint, extra_tables)) => {
                // We try to build the db from a checkpoint in a temporary dir.
                let mut tmp_path = self.path.clone();
                tmp_path.pop();
                tmp_path.push("tmp");
                if tmp_path.exists() {
                    fs::remove_dir_all(&tmp_path)?;
                }
                fs::create_dir_all(&tmp_path)?;
                let (_db, column_names) = build_db_from_checkpoint(
                    &tmp_path,
                    hash,
                    checkpoint,
                    extra_tables,
                    self.options.clone(),
                )?;
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
                if self.read_only {
                    DB::open_cf_descriptors_read_only(&options, &self.path, cf_iter, false)?
                } else {
                    DB::open_cf_descriptors(&options, &self.path, cf_iter)?
                }
            },
            None => {
                if self.read_only {
                    DB::open_cf_descriptors_read_only(&self.options, self.path, cf_iter, false)?
                } else {
                    DB::open_cf_descriptors(&self.options, self.path, cf_iter)?
                }
            },
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
    pub fn serialize(&self, exclude_tables: &[String]) -> Vec<u8> {
        let exclude_tables: HashSet<String> = exclude_tables.iter().cloned().collect();
        let tables = self
            .columns
            .clone()
            .iter()
            .map(|table| table.to_string())
            .filter(|table| !exclude_tables.contains(table))
            .collect::<Vec<_>>();

        // We can safely unwrap here.
        // This will only panic if the table names in `columns` are not consistent with the
        // database.
        serialize_db(&self.db, &tables).unwrap()
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

    fn keys(&self, tid: u8) -> Box<dyn Iterator<Item = BoxedVec> + '_> {
        let cf = self.db.cf_handle(&self.columns[tid as usize]).unwrap();
        Box::new(
            self.db
                .iterator_cf(&cf, rocksdb::IteratorMode::Start)
                .map(|res| {
                    res.expect("failed to get entry from column family iterator")
                        .0
                }),
        )
    }

    fn get_all(&self, tid: u8) -> Box<dyn Iterator<Item = (BoxedVec, BoxedVec)> + '_> {
        let cf = self.db.cf_handle(&self.columns[tid as usize]).unwrap();
        Box::new(
            self.db
                .iterator_cf(&cf, rocksdb::IteratorMode::Start)
                .map(|res| {
                    let (key, value) =
                        res.expect("failed to get entry from column family iterator");
                    (key, value)
                }),
        )
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use atomo::{DefaultSerdeBackend, SerdeBackend};
    use rocksdb::Options;
    use tempfile::tempdir;

    use crate::serialization::deserialize_db;
    use crate::{AtomoBuilderWithRocks, RocksBackend, RocksBackendBuilder};

    #[test]
    fn test_serialize() {
        let temp_dir = tempdir().unwrap();

        // Setup builder.
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);
        let rocksdb = RocksBackendBuilder::new(temp_dir.path()).with_options(options);

        // Build db.
        let mut db = AtomoBuilderWithRocks::new(rocksdb)
            .with_table::<u32, String>("data")
            .with_table::<u32, String>("tree")
            .enable_iter("data")
            .enable_iter("tree")
            .build()
            .unwrap();

        // Insert some data.
        db.run(
            |ctx: &mut atomo::TableSelector<RocksBackend, atomo::BincodeSerde>| {
                let mut data_table = ctx.get_table::<u32, String>("data");
                data_table.insert(1, "one".to_string());
                data_table.insert(2, "two".to_string());
                data_table.insert(3, "three".to_string());
            },
        );

        // Serialize the db.
        let (bytes1, hash1) = {
            let storage = db.get_storage_backend_unsafe();

            let bytes = storage.serialize(&["tree".to_string()]);
            let hash = fleek_blake3::hash(&bytes);

            (bytes, hash)
        };

        // Insert some excluded table data.
        db.run(
            |ctx: &mut atomo::TableSelector<RocksBackend, atomo::BincodeSerde>| {
                let mut tree_table = ctx.get_table::<u32, String>("tree");
                tree_table.insert(101, "foo".to_string());
                tree_table.insert(102, "bar".to_string());
                tree_table.insert(103, "baz".to_string());
            },
        );

        // Serialize the db, excluding the tree table.
        let (bytes2, hash2) = {
            let storage = db.get_storage_backend_unsafe();

            let bytes = storage.serialize(&["tree".to_string()]);
            let hash = fleek_blake3::hash(&bytes);

            (bytes, hash)
        };

        // Check that the serialized bytes and hash are the same for both serializations.
        assert_eq!(bytes1, bytes2);
        assert_eq!(hash1, hash2);

        // Deserialize the db.
        let tables = deserialize_db(&bytes1).unwrap();

        // Check that the deserialized db has the same data as the original db.
        assert_eq!(tables.len(), 1);
        assert_eq!(tables.get("data").unwrap().len(), 3);
        assert!(!tables.contains_key("tree"));

        let data = tables
            .get("data")
            .unwrap()
            .iter()
            .map(|(k, v)| {
                (
                    DefaultSerdeBackend::deserialize::<u32>(k),
                    DefaultSerdeBackend::deserialize::<String>(v),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            data,
            vec![
                (1, "one".to_string()),
                (2, "two".to_string()),
                (3, "three".to_string())
            ]
        );
    }

    #[test]
    fn create_insert_and_query() {
        let temp_dir = tempdir().unwrap();

        // setup rocksdb
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);
        let rocksdb = RocksBackendBuilder::new(temp_dir.path()).with_options(options);

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
    }
}
