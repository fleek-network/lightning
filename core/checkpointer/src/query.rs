use std::collections::HashSet;

use lightning_interfaces::types::{AggregateCheckpointHeader, CheckpointHeader, Epoch};
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
    fn get_checkpoint_headers(&self, epoch: Epoch) -> HashSet<CheckpointHeader> {
        self.db.get_checkpoint_headers(epoch)
    }

    fn get_aggregate_checkpoint_header(&self, epoch: Epoch) -> Option<AggregateCheckpointHeader> {
        self.db.get_aggregate_checkpoint_header(epoch)
    }
}
