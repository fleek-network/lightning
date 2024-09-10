use std::collections::HashMap;

use atomo::{Atomo, DefaultSerdeBackend, QueryPerm};
use atomo_rocks::RocksBackend;
use lightning_interfaces::types::{AggregateCheckpoint, CheckpointAttestation, Epoch, NodeIndex};

use super::database::{AGGREGATE_CHECKPOINT_HEADERS_TABLE, NODE_CHECKPOINT_HEADERS_TABLE};
use crate::database::CheckpointerDatabaseQuery;

/// A checkpointer database query type that uses RocksDB as the underlying datastore.
#[derive(Clone)]
pub struct RocksCheckpointerDatabaseQuery {
    atomo: Atomo<QueryPerm, RocksBackend, DefaultSerdeBackend>,
}

impl RocksCheckpointerDatabaseQuery {
    pub fn new(atomo: Atomo<QueryPerm, RocksBackend, DefaultSerdeBackend>) -> Self {
        Self { atomo }
    }
}

impl CheckpointerDatabaseQuery for RocksCheckpointerDatabaseQuery {
    fn get_checkpoint_attestations(
        &self,
        epoch: Epoch,
    ) -> HashMap<NodeIndex, CheckpointAttestation> {
        self.atomo.query().run(|ctx| {
            let table = ctx.get_table::<Epoch, HashMap<NodeIndex, CheckpointAttestation>>(
                NODE_CHECKPOINT_HEADERS_TABLE,
            );

            table.get(epoch).unwrap_or_default()
        })
    }

    fn get_node_checkpoint_attestation(
        &self,
        epoch: Epoch,
        node_id: NodeIndex,
    ) -> Option<CheckpointAttestation> {
        self.get_checkpoint_attestations(epoch)
            .get(&node_id)
            .cloned()
    }

    fn get_aggregate_checkpoint(&self, epoch: Epoch) -> Option<AggregateCheckpoint> {
        self.atomo.query().run(|ctx| {
            let table =
                ctx.get_table::<Epoch, AggregateCheckpoint>(AGGREGATE_CHECKPOINT_HEADERS_TABLE);

            table.get(epoch)
        })
    }
}
