use std::collections::HashMap;

use lightning_interfaces::types::{AggregateCheckpoint, CheckpointAttestation, Epoch, NodeIndex};
use lightning_interfaces::CheckpointerQueryInterface;

use crate::database::CheckpointerDatabaseQuery;
use crate::rocks::RocksCheckpointerDatabaseQuery;

#[derive(Clone)]
pub struct CheckpointerQuery {
    db: RocksCheckpointerDatabaseQuery,
}

impl CheckpointerQuery {
    pub fn new(db: RocksCheckpointerDatabaseQuery) -> Self {
        Self { db }
    }
}

impl CheckpointerQueryInterface for CheckpointerQuery {
    fn get_checkpoint_attestations(
        &self,
        epoch: Epoch,
    ) -> HashMap<NodeIndex, CheckpointAttestation> {
        self.db.get_checkpoint_attestations(epoch)
    }

    fn get_aggregate_checkpoint(&self, epoch: Epoch) -> Option<AggregateCheckpoint> {
        self.db.get_aggregate_checkpoint(epoch)
    }
}
