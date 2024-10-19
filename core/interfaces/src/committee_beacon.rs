use std::collections::HashMap;

use fdi::BuildGraph;
use lightning_types::{CommitteeSelectionBeaconCommit, CommitteeSelectionBeaconReveal};

use crate::components::NodeComponents;

/// The committee beacon component participates in a commit-reveal protocol that contributes to a
/// random seed that is used to select a committee for the next epoch.
#[interfaces_proc::blank]
pub trait CommitteeBeaconInterface<C: NodeComponents>: BuildGraph + Send + Sync {
    /// The query type for the committee beacon component.
    type Query: CommitteeBeaconQueryInterface;

    /// Get a query instance for the committee beacon component.
    fn query(&self) -> Self::Query;
}

/// Query type for the committee beacon component.
#[interfaces_proc::blank]
pub trait CommitteeBeaconQueryInterface: Clone + Send + Sync + 'static {
    /// Get all locally stored beacons.
    fn get_beacons(
        &self,
    ) -> HashMap<CommitteeSelectionBeaconCommit, CommitteeSelectionBeaconReveal>;
}
