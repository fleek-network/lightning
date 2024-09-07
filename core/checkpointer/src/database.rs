use std::collections::HashMap;

use lightning_interfaces::types::{AggregateCheckpointHeader, CheckpointHeader, Epoch, NodeIndex};

use crate::config::CheckpointerDatabaseConfig;

/// A trait for a checkpointer database, encapsulating the database operations that the
/// checkpointer needs to perform.
///
/// These operations are intentionally specific to uses within the checkpointer. They should
/// encapsulate any consistency needs internally to the implementation.
///
/// It is expected that implementations are thread-safe and can be shared between multiple
/// threads.
pub trait CheckpointerDatabase: Clone + Send + Sync {
    /// The database reader type.
    type Query: CheckpointerDatabaseQuery;

    /// Build a new database instance using the given configuration.
    fn build(config: CheckpointerDatabaseConfig) -> Self;

    /// Get the query instance for this database.
    fn query(&self) -> Self::Query;

    /// Set the checkpoint header for the given epoch and node.
    fn set_node_checkpoint_header(&self, epoch: Epoch, header: CheckpointHeader);

    /// Set the aggregate checkpoint header for the given epoch.
    ///
    /// There is just a single one of these per epoch, and any existing entry for the epoch will
    /// be overwritten.
    fn set_aggregate_checkpoint_header(&self, epoch: Epoch, header: AggregateCheckpointHeader);
}

/// A trait for a checkpointer database query, encapsulating the database query operations that
/// the checkpointer needs to perform.
///
/// There can be many query instances for a given database, and they can be shared between
/// multiple threads.
pub trait CheckpointerDatabaseQuery {
    /// Get the map of checkpoint headers by node for the given epoch.
    fn get_checkpoint_headers(&self, epoch: Epoch) -> HashMap<NodeIndex, CheckpointHeader>;

    /// Get the checkpoint header for the given epoch and node.
    fn get_node_checkpoint_header(
        &self,
        epoch: Epoch,
        node_id: NodeIndex,
    ) -> Option<CheckpointHeader>;

    /// Get the aggregate checkpoint header for the given epoch.
    fn get_aggregate_checkpoint_header(&self, epoch: Epoch) -> Option<AggregateCheckpointHeader>;
}
