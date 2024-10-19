use std::collections::HashMap;

use atomo::{Atomo, DefaultSerdeBackend, QueryPerm};
use atomo_rocks::RocksBackend;
use lightning_interfaces::types::{CommitteeSelectionBeaconCommit, CommitteeSelectionBeaconReveal};

use super::database::BEACONS_TABLE;
use crate::database::CommitteeBeaconDatabaseQuery;

/// A committee beacon database query type that uses RocksDB as the underlying datastore.
#[derive(Clone)]
pub struct RocksCommitteeBeaconDatabaseQuery {
    atomo: Atomo<QueryPerm, RocksBackend, DefaultSerdeBackend>,
}

impl RocksCommitteeBeaconDatabaseQuery {
    pub fn new(atomo: Atomo<QueryPerm, RocksBackend, DefaultSerdeBackend>) -> Self {
        Self { atomo }
    }
}

impl CommitteeBeaconDatabaseQuery for RocksCommitteeBeaconDatabaseQuery {
    fn get_beacon(
        &self,
        commit: CommitteeSelectionBeaconCommit,
    ) -> Option<CommitteeSelectionBeaconReveal> {
        self.atomo.query().run(|ctx| {
            let table = ctx
                .get_table::<CommitteeSelectionBeaconCommit, CommitteeSelectionBeaconReveal>(
                    BEACONS_TABLE,
                );

            table.get(commit)
        })
    }

    fn get_beacons(
        &self,
    ) -> HashMap<CommitteeSelectionBeaconCommit, CommitteeSelectionBeaconReveal> {
        self.atomo.query().run(|ctx| {
            let table = ctx
                .get_table::<CommitteeSelectionBeaconCommit, CommitteeSelectionBeaconReveal>(
                    BEACONS_TABLE,
                );

            table.as_map()
        })
    }
}
