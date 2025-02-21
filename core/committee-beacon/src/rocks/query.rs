use atomo::{Atomo, DefaultSerdeBackend, QueryPerm};
use atomo_rocks::RocksBackend;
use fxhash::FxHashMap;
use lightning_interfaces::types::{
    CommitteeSelectionBeaconCommit,
    CommitteeSelectionBeaconReveal,
    CommitteeSelectionBeaconRound,
    Epoch,
};

use super::database::{BeaconsTableKey, CommitsTableKey, BEACONS_TABLE, COMMITS_TABLE};
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
        epoch: Epoch,
        commit: CommitteeSelectionBeaconCommit,
    ) -> Option<CommitteeSelectionBeaconReveal> {
        self.atomo.query().run(|ctx| {
            let table =
                ctx.get_table::<BeaconsTableKey, CommitteeSelectionBeaconReveal>(BEACONS_TABLE);

            table.get((epoch, commit))
        })
    }

    fn get_commit(
        &self,
        epoch: Epoch,
        round: CommitteeSelectionBeaconRound,
    ) -> Option<CommitteeSelectionBeaconCommit> {
        self.atomo.query().run(|ctx| {
            let table =
                ctx.get_table::<CommitsTableKey, CommitteeSelectionBeaconCommit>(COMMITS_TABLE);

            table.get((epoch, round))
        })
    }

    fn get_beacons(
        &self,
    ) -> FxHashMap<(Epoch, CommitteeSelectionBeaconCommit), CommitteeSelectionBeaconReveal> {
        self.atomo.query().run(|ctx| {
            let table =
                ctx.get_table::<BeaconsTableKey, CommitteeSelectionBeaconReveal>(BEACONS_TABLE);

            table.as_map()
        })
    }
}
