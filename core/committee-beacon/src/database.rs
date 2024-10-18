use std::collections::HashMap;

use lightning_interfaces::types::{CommitteeSelectionBeaconCommit, CommitteeSelectionBeaconReveal};

use crate::config::CommitteeBeaconDatabaseConfig;

/// A trait for a committee beacon database, encapsulating the database operations that the
/// committee beacon needs to perform.
///
/// These operations are intentionally specific to uses within the committee beacon. They should
/// encapsulate any consistency needs internally to the implementation.
///
/// It is expected that implementations are thread-safe and can be shared between multiple
/// threads.
pub trait CommitteeBeaconDatabase: Clone + Send + Sync {
    /// The database reader type.
    type Query: CommitteeBeaconDatabaseQuery;

    /// Build a new database instance using the given configuration.
    fn build(config: CommitteeBeaconDatabaseConfig) -> Self;

    /// Get the query instance for this database.
    fn query(&self) -> Self::Query;

    /// Set the beacon reveal value for a given commit.
    fn set_beacon(
        &self,
        commit: CommitteeSelectionBeaconCommit,
        reveal: CommitteeSelectionBeaconReveal,
    );

    /// Clear beacons.
    fn clear_beacons(&self);
}

/// A trait for a committee beacon database query, encapsulating the database query operations that
/// the committee beacon needs to perform.
///
/// There can be many query instances for a given database, and they can be shared between
/// multiple threads.
pub trait CommitteeBeaconDatabaseQuery {
    /// Get beacon for a given commit.
    fn get_beacon(
        &self,
        commit: CommitteeSelectionBeaconCommit,
    ) -> Option<CommitteeSelectionBeaconReveal>;

    /// Get all the locally stored beacons.
    fn get_beacons(
        &self,
    ) -> HashMap<CommitteeSelectionBeaconCommit, CommitteeSelectionBeaconReveal>;
}
