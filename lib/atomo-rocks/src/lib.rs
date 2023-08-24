//! A [`rocksdb`] storage backend implementation for [`atomo`].

use std::path::PathBuf;

use atomo::batch::Operation;
use atomo::{AtomoBuilder, DefaultSerdeBackend, StorageBackend, StorageBackendConstructor};
/// Re-export of [`rocksdb::Options`].
pub use rocksdb::Options;
use rocksdb::WriteBatch;

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
/// let path = "test-rocksdb";
/// let mut options = Options::default();
/// options.create_if_missing(true);
/// let rocksdb = RocksBackendBuilder::new(path).with_options(options);
/// let atomo = AtomoBuilderWithRocks::<DefaultSerdeBackend>::new(rocksdb).build();
///
/// // cleanup
/// drop(atomo);
/// std::fs::remove_dir_all(path).unwrap();
/// ```
pub struct RocksBackendBuilder {
    path: PathBuf,
    columns: Vec<String>,
    options: rocksdb::Options,
}

impl RocksBackendBuilder {
    /// Create a new builder at the given path.
    #[inline(always)]
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        Self {
            path: path.into(),
            columns: Default::default(),
            options: Default::default(),
        }
    }

    /// Provide a [`rocksdb::Options`] object to the builder.
    #[inline(always)]
    pub fn with_options(mut self, opts: rocksdb::Options) -> Self {
        self.options = opts;
        self
    }
}

impl StorageBackendConstructor for RocksBackendBuilder {
    type Storage = RocksBackend;

    type Error = rocksdb::Error;

    fn open_table(&mut self, name: String) {
        self.columns.push(name)
    }

    fn build(self) -> Result<Self::Storage, Self::Error> {
        Ok(RocksBackend {
            columns: self.columns.clone(),
            db: rocksdb::DB::open_cf(&self.options, self.path, self.columns)?,
        })
    }
}

/// RocksDB persistence backend for [`atomo`].
pub struct RocksBackend {
    db: rocksdb::DB,
    columns: Vec<String>,
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
        self.db.get_cf(&cf, key).ok().flatten()
    }

    fn contains(&self, tid: u8, key: &[u8]) -> bool {
        let cf = self.db.cf_handle(&self.columns[tid as usize]).unwrap();
        self.db.key_may_exist_cf(&cf, key)
    }
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
            .with_table::<u64, u64>("name-of-table")
            .build();

        let query_runner = db.query();
        let table_res = db.resolve::<u64, u64>("name-of-table");

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

        std::fs::remove_dir_all(path).expect("failed to remove old rocksdb");
    }
}
