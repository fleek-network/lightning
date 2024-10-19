use std::sync::{Arc, Mutex};

use atomo::{Atomo, AtomoBuilder, DefaultSerdeBackend, UpdatePerm};
use atomo_rocks::{Options, RocksBackend, RocksBackendBuilder};
use lightning_interfaces::types::{CommitteeSelectionBeaconCommit, CommitteeSelectionBeaconReveal};

use super::RocksCommitteeBeaconDatabaseQuery;
use crate::config::CommitteeBeaconDatabaseConfig;
use crate::database::CommitteeBeaconDatabase;

pub(crate) const BEACONS_TABLE: &str = "beacons";

/// A committee beacon database writer that uses RocksDB as the underlying datastore.
///
/// It is thread-safe and can be shared between multiple threads.
#[derive(Clone)]
pub struct RocksCommitteeBeaconDatabase {
    atomo: Arc<Mutex<Atomo<UpdatePerm, RocksBackend, DefaultSerdeBackend>>>,
}

impl RocksCommitteeBeaconDatabase {
    pub fn new(atomo: Arc<Mutex<Atomo<UpdatePerm, RocksBackend, DefaultSerdeBackend>>>) -> Self {
        Self { atomo }
    }
}

impl CommitteeBeaconDatabase for RocksCommitteeBeaconDatabase {
    type Query = RocksCommitteeBeaconDatabaseQuery;

    fn build(config: CommitteeBeaconDatabaseConfig) -> Self {
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);

        let builder = RocksBackendBuilder::new(config.path.to_path_buf()).with_options(options);
        let builder = AtomoBuilder::new(builder)
            .with_table::<CommitteeSelectionBeaconCommit, CommitteeSelectionBeaconReveal>(
                BEACONS_TABLE,
            )
            .enable_iter(BEACONS_TABLE);

        let db = builder.build().unwrap();
        let db = Arc::new(Mutex::new(db));

        Self::new(db)
    }

    fn query(&self) -> RocksCommitteeBeaconDatabaseQuery {
        RocksCommitteeBeaconDatabaseQuery::new(self.atomo.lock().unwrap().query())
    }

    fn set_beacon(
        &self,
        commit: CommitteeSelectionBeaconCommit,
        reveal: CommitteeSelectionBeaconReveal,
    ) {
        self.atomo.lock().unwrap().run(|ctx| {
            let mut table = ctx
                .get_table::<CommitteeSelectionBeaconCommit, CommitteeSelectionBeaconReveal>(
                    BEACONS_TABLE,
                );

            table.insert(commit, reveal);
        });
    }

    fn clear_beacons(&self) {
        self.atomo.lock().unwrap().run(|ctx| {
            let mut table = ctx
                .get_table::<CommitteeSelectionBeaconCommit, CommitteeSelectionBeaconReveal>(
                    BEACONS_TABLE,
                );

            for key in table.keys() {
                table.remove(key);
            }
        });
    }
}
