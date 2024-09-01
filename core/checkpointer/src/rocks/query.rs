use std::collections::HashSet;

use atomo::{Atomo, DefaultSerdeBackend, QueryPerm};
use atomo_rocks::RocksBackend;
use lightning_interfaces::types::{AggregateCheckpointHeader, CheckpointHeader, Epoch};

use crate::database::CheckpointerDatabaseQuery;

const CHECKPOINT_HEADERS_TABLE: &str = "checkpoint_headers";
const AGGREGATE_CHECKPOINT_HEADERS_TABLE: &str = "aggregate_checkpoint_headers";

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
    fn get_checkpoint_headers(&self, epoch: Epoch) -> HashSet<CheckpointHeader> {
        self.atomo.query().run(|ctx| {
            let table = ctx.get_table::<Epoch, HashSet<CheckpointHeader>>(CHECKPOINT_HEADERS_TABLE);

            table.get(epoch).unwrap_or_default()
        })
    }

    fn get_aggregate_checkpoint_header(&self, epoch: Epoch) -> Option<AggregateCheckpointHeader> {
        self.atomo.query().run(|ctx| {
            let table = ctx
                .get_table::<Epoch, AggregateCheckpointHeader>(AGGREGATE_CHECKPOINT_HEADERS_TABLE);

            table.get(epoch)
        })
    }
}
