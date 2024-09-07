use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use atomo::{Atomo, AtomoBuilder, DefaultSerdeBackend, UpdatePerm};
use atomo_rocks::{Options, RocksBackend, RocksBackendBuilder};
use lightning_interfaces::types::{AggregateCheckpointHeader, CheckpointHeader, Epoch, NodeIndex};

use super::query::RocksCheckpointerDatabaseQuery;
use crate::database::CheckpointerDatabase;
use crate::CheckpointerDatabaseConfig;

pub(crate) const NODE_CHECKPOINT_HEADERS_TABLE: &str = "node_checkpoint_headers";
pub(crate) const AGGREGATE_CHECKPOINT_HEADERS_TABLE: &str = "aggregate_checkpoint_headers";

/// A checkpointer database writer that uses RocksDB as the underlying datastore.
///
/// It is thread-safe and can be shared between multiple threads.
#[derive(Clone)]
pub struct RocksCheckpointerDatabase {
    atomo: Arc<Mutex<Atomo<UpdatePerm, RocksBackend, DefaultSerdeBackend>>>,
}

impl RocksCheckpointerDatabase {
    pub fn new(atomo: Arc<Mutex<Atomo<UpdatePerm, RocksBackend, DefaultSerdeBackend>>>) -> Self {
        Self { atomo }
    }
}

impl CheckpointerDatabase for RocksCheckpointerDatabase {
    type Query = RocksCheckpointerDatabaseQuery;

    fn build(config: CheckpointerDatabaseConfig) -> Self {
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);

        let builder = RocksBackendBuilder::new(config.path.to_path_buf()).with_options(options);
        let builder = AtomoBuilder::new(builder)
            .with_table::<Epoch, HashMap<NodeIndex, CheckpointHeader>>(
                NODE_CHECKPOINT_HEADERS_TABLE,
            )
            .with_table::<Epoch, AggregateCheckpointHeader>(AGGREGATE_CHECKPOINT_HEADERS_TABLE);

        let db = builder.build().unwrap();
        let db = Arc::new(Mutex::new(db));

        Self::new(db)
    }

    fn query(&self) -> RocksCheckpointerDatabaseQuery {
        RocksCheckpointerDatabaseQuery::new(self.atomo.lock().unwrap().query())
    }

    fn set_node_checkpoint_header(&self, epoch: Epoch, header: CheckpointHeader) {
        self.atomo.lock().unwrap().run(|ctx| {
            let mut table = ctx.get_table::<Epoch, HashMap<NodeIndex, CheckpointHeader>>(
                NODE_CHECKPOINT_HEADERS_TABLE,
            );

            let mut headers = table.get(epoch).unwrap_or_default();
            headers.insert(header.node_id, header);
            table.insert(epoch, headers);
        });
    }

    fn set_aggregate_checkpoint_header(&self, epoch: Epoch, header: AggregateCheckpointHeader) {
        self.atomo.lock().unwrap().run(|ctx| {
            let mut table = ctx
                .get_table::<Epoch, AggregateCheckpointHeader>(AGGREGATE_CHECKPOINT_HEADERS_TABLE);

            table.insert(epoch, header);
        });
    }
}
