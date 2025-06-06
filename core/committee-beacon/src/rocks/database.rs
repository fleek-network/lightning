use std::sync::{Arc, Mutex};

use atomo::{Atomo, AtomoBuilder, DefaultSerdeBackend, UpdatePerm};
use atomo_rocks::{Options, RocksBackend, RocksBackendBuilder};
use lightning_interfaces::types::{
    CommitteeSelectionBeaconCommit,
    CommitteeSelectionBeaconReveal,
    CommitteeSelectionBeaconRound,
    Epoch,
};

use super::RocksCommitteeBeaconDatabaseQuery;
use crate::config::CommitteeBeaconDatabaseConfig;
use crate::database::CommitteeBeaconDatabase;

pub(crate) const BEACONS_TABLE: &str = "beacons";
pub(crate) const COMMITS_TABLE: &str = "commits";

pub type BeaconsTableKey = (Epoch, CommitteeSelectionBeaconCommit);
pub type CommitsTableKey = (Epoch, CommitteeSelectionBeaconRound);

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
            .with_table::<(Epoch, CommitteeSelectionBeaconCommit), CommitteeSelectionBeaconReveal>(
                BEACONS_TABLE,
            )
            .with_table::<(Epoch, CommitteeSelectionBeaconRound), CommitteeSelectionBeaconCommit>(
                COMMITS_TABLE,
            )
            .enable_iter(BEACONS_TABLE)
            .enable_iter(COMMITS_TABLE);

        let db = builder.build().unwrap();
        let db = Arc::new(Mutex::new(db));

        Self::new(db)
    }

    fn query(&self) -> RocksCommitteeBeaconDatabaseQuery {
        RocksCommitteeBeaconDatabaseQuery::new(self.atomo.lock().unwrap().query())
    }

    fn set_beacon(
        &self,
        epoch: Epoch,
        round: CommitteeSelectionBeaconRound,
        commit: CommitteeSelectionBeaconCommit,
        reveal: CommitteeSelectionBeaconReveal,
    ) {
        self.atomo.lock().unwrap().run(|ctx| {
            let mut table =
                ctx.get_table::<BeaconsTableKey, CommitteeSelectionBeaconReveal>(BEACONS_TABLE);
            table.insert((epoch, commit), reveal);

            let mut table =
                ctx.get_table::<CommitsTableKey, CommitteeSelectionBeaconCommit>(COMMITS_TABLE);
            table.insert((epoch, round), commit);
        });
    }

    fn clear_beacons_before_epoch(&self, before_epoch: Epoch) {
        self.atomo.lock().unwrap().run(|ctx| {
            let mut table =
                ctx.get_table::<BeaconsTableKey, CommitteeSelectionBeaconReveal>(BEACONS_TABLE);

            for (epoch, commit) in table.keys() {
                if epoch < before_epoch {
                    table.remove((epoch, commit));
                }
            }

            let mut table =
                ctx.get_table::<CommitsTableKey, CommitteeSelectionBeaconCommit>(COMMITS_TABLE);
            for (epoch, round) in table.keys() {
                if epoch < before_epoch {
                    table.remove((epoch, round));
                }
            }
        });
    }
}
