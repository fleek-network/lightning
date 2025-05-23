use fxhash::FxHashMap;
use lightning_interfaces::types::{
    CommitteeSelectionBeaconCommit,
    CommitteeSelectionBeaconReveal,
    Epoch,
};
use lightning_interfaces::CommitteeBeaconQueryInterface;

use crate::database::CommitteeBeaconDatabaseQuery;
use crate::rocks::RocksCommitteeBeaconDatabaseQuery;

#[derive(Clone)]
pub struct CommitteeBeaconQuery {
    db: RocksCommitteeBeaconDatabaseQuery,
}

impl CommitteeBeaconQuery {
    pub fn new(db: RocksCommitteeBeaconDatabaseQuery) -> Self {
        Self { db }
    }
}

impl CommitteeBeaconQueryInterface for CommitteeBeaconQuery {
    fn get_beacons(
        &self,
    ) -> FxHashMap<(Epoch, CommitteeSelectionBeaconCommit), CommitteeSelectionBeaconReveal> {
        self.db.get_beacons()
    }
}
