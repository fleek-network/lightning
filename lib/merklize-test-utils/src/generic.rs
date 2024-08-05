use atomo_rocks::{Options, RocksBackendBuilder};
use tempfile::TempDir;

pub const DATA_COUNT_SIMPLE: usize = 10;
pub const DATA_COUNT_MEDIUM: usize = 100;
pub const DATA_COUNT_COMPLEX: usize = 1000;

pub fn rocksdb_builder(temp_dir: &TempDir) -> RocksBackendBuilder {
    let mut options = Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);

    RocksBackendBuilder::new(temp_dir.path()).with_options(options)
}
