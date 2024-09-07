use std::collections::{HashMap, HashSet};

use anyhow::Result;
use bit_set::BitSet;
use fleek_crypto::ConsensusAggregateSignature;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{AggregateCheckpointHeader, CheckpointHeader, Epoch, NodeIndex};
use lightning_utils::application::QueryRunnerExt;
use merklize::StateRootHash;
use types::NodeInfo;

use crate::database::{CheckpointerDatabase, CheckpointerDatabaseQuery};
use crate::rocks::RocksCheckpointerDatabase;
pub struct AggregateCheckpointBuilder<C: Collection> {
    db: RocksCheckpointerDatabase,
    app_query: c!(C::ApplicationInterface::SyncExecutor),
}

impl<C: Collection> AggregateCheckpointBuilder<C> {
    pub fn new(
        db: RocksCheckpointerDatabase,
        app_query: c!(C::ApplicationInterface::SyncExecutor),
    ) -> Self {
        Self { db, app_query }
    }

    /// Returns the eligible node set for checking supermajority.
    pub fn get_eligible_nodes(&self) -> HashMap<NodeIndex, NodeInfo> {
        self.app_query
            .get_active_nodes()
            .iter()
            .map(|node| (node.index, node.info.clone()))
            .collect()
    }

    // Check if we have a supermajority of attestations that are in agreement on the next state root
    // for the epoch. If so, build an aggregate checkpoint header, and save it to the local
    // database.
    //
    // We assume that the checkpoint header signatures have been validated and deduplicated by the
    // time they reach this point.
    pub fn build_and_save_aggregate_if_supermajority(
        &self,
        epoch: Epoch,
        nodes_count: usize,
    ) -> Result<()> {
        let headers = self.db.query().get_checkpoint_headers(epoch);

        let mut headers_by_state_root = HashMap::new();
        for header in headers.values() {
            headers_by_state_root
                .entry(header.next_state_root)
                .or_insert_with(HashSet::new)
                .insert(header);
            let state_root_headers = &headers_by_state_root[&header.next_state_root];

            // Check for supermajority.
            if state_root_headers.len() > (2 * nodes_count) / 3 {
                tracing::info!("checkpoint supermajority reached for epoch {}", epoch);

                // We have a supermajority of attestations in agreement for the epoch.
                let aggregate_header = self.build_aggregate_checkpoint_header(
                    epoch,
                    header.next_state_root,
                    state_root_headers,
                )?;

                // Save the aggregate signature to the local database.
                self.db
                    .set_aggregate_checkpoint_header(epoch, aggregate_header);

                break;
            } else {
                tracing::debug!("missing supermajority of checkpoints for epoch {}", epoch);
            }
        }

        Ok(())
    }

    fn build_aggregate_checkpoint_header(
        &self,
        epoch: Epoch,
        state_root: StateRootHash,
        state_root_headers: &HashSet<&CheckpointHeader>,
    ) -> Result<AggregateCheckpointHeader> {
        // Aggregate the signatures.
        let signatures = state_root_headers
            .iter()
            .map(|header| header.signature)
            .collect::<Vec<_>>();
        let aggregate_signature = ConsensusAggregateSignature::aggregate(signatures.iter())
            .map_err(|e| anyhow::anyhow!(e))?;

        // Build the nodes bit set.
        let nodes = BitSet::<NodeIndex>::from_iter(
            state_root_headers
                .iter()
                .map(|header| header.node_id as usize),
        );

        // Create the aggregate checkpoint header.
        let aggregate_header = AggregateCheckpointHeader {
            epoch,
            state_root,
            signature: aggregate_signature,
            nodes,
        };

        Ok(aggregate_header)
    }
}
