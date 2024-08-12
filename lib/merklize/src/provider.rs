use std::collections::HashMap;

use anyhow::Result;
use atomo::batch::Operation;
use atomo::{
    Atomo,
    AtomoBuilder,
    SerdeBackend,
    StorageBackend,
    StorageBackendConstructor,
    TableId,
    TableSelector,
    UpdatePerm,
};
use fxhash::FxHashMap;
use tracing::trace_span;

use crate::{SimpleHasher, StateProof, StateRootHash};

/// A trait for a merklize provider used to maintain and interact with the state tree.
///
/// ## Examples
///
/// ```rust
#[doc = include_str!("../examples/jmt-sha256.rs")]
/// ```
pub trait MerklizeProvider {
    type Storage: StorageBackend;
    type Serde: SerdeBackend;
    type Hasher: SimpleHasher;
    type Proof: StateProof;

    /// Augment the provided atomo builder with the necessary tables for the merklize provider.
    ///
    /// Arguments:
    /// - `builder`: The atomo builder to augment.
    fn with_tables<C: StorageBackendConstructor>(
        builder: AtomoBuilder<C, Self::Serde>,
    ) -> AtomoBuilder<C, Self::Serde>;

    /// Returns the root hash of the state tree.
    ///
    /// Arguments:
    /// - `ctx`: The atomo execution context that will be used to get the root hash of the state
    ///   tree.
    fn get_state_root(ctx: &TableSelector<Self::Storage, Self::Serde>) -> Result<StateRootHash>;

    /// Generates and returns a merkle proof for the given key in the state.
    ///
    /// This method uses an atomo execution context, so it is safe to use concurrently.
    ///
    /// Arguments:
    /// - `ctx`: The atomo execution context that will be used to generate the proof.
    /// - `table`: The name of the table to generate the proof for.
    /// - `serialized_key`: The serialized key to generate the proof for.
    fn get_state_proof(
        ctx: &TableSelector<Self::Storage, Self::Serde>,
        table: &str,
        serialized_key: Vec<u8>,
    ) -> Result<Self::Proof>;

    /// Applies the changes in the given batch of updates to the state tree.
    ///
    /// This method uses an atomo execution context, so it is safe to use concurrently.
    ///
    /// Arguments:
    /// - `ctx`: The atomo execution context that will be used to apply the changes.
    /// - `batch`: The batch of pending changes to apply to the state tree.
    fn update_state_tree<I>(
        ctx: &TableSelector<Self::Storage, Self::Serde>,
        batch: HashMap<String, I>,
    ) -> Result<()>
    where
        I: Iterator<Item = (Box<[u8]>, Operation)>;

    /// Clears the existing state tree data. This does not delete or modify any of the state data,
    /// just the tree structure and tables related to it.
    ///
    /// This method acts directly on the atomo database instance, so it should only be used with
    /// caution in isolation for use cases such as backfilling the state tree during rollout or
    /// corruption recovery.
    ///
    /// Arguments:
    /// - `db`: The atomo database instance to use for clearing the state tree.
    // TODO(snormore): Add better test and benchmark coverage for this.
    fn clear_state_tree(db: &mut Atomo<UpdatePerm, Self::Storage, Self::Serde>) -> Result<()>;

    /// Applies the pending changes in the given context to the state tree.
    /// This is an implementation that makes use of the `update_state_tree` method, passing it the
    /// batch of pending changes from the context.
    ///
    /// Arguments:
    /// - `ctx`: The atomo execution context that will be used to get the pending changes and apply
    ///   them to the state tree.
    // TODO(snormore): Add better test and benchmark coverage for this.
    fn update_state_tree_from_context(
        ctx: &TableSelector<Self::Storage, Self::Serde>,
    ) -> Result<()> {
        let span = trace_span!("update_state_tree_from_context");
        let _enter = span.enter();

        let mut table_name_by_id = FxHashMap::default();
        for (i, table) in ctx.tables().into_iter().enumerate() {
            table_name_by_id.insert(i as TableId, table);
        }

        let batch = ctx
            .batch()
            .into_raw()
            .into_iter()
            .enumerate()
            .map(|(tid, changes)| {
                let table = table_name_by_id.get(&(tid as TableId)).unwrap().clone();
                let changes = changes.into_iter().map(|(k, v)| (k.clone(), v.clone()));
                (table, changes)
            })
            .collect();

        Self::update_state_tree(ctx, batch)
    }

    /// Clears existing state tree and rebuilds it from scratch. This does not delete or modify any
    /// of the state data, just the tree structure and tables related to it. The tree is then
    /// rebuilt by applying all of the state data in the atomo context to the new tree.
    ///
    /// This method acts directly on the atomo database instance, so it should only be used with
    /// caution in isolation for use cases such as backfilling the state tree during rollout or
    /// corruption recovery.
    ///
    /// Arguments:
    /// - `db`: The atomo database instance to use for clearing and rebuilding the state tree.
    // TODO(snormore): Add better test and benchmark coverage for this.
    fn clear_and_rebuild_state_tree(
        db: &mut Atomo<UpdatePerm, Self::Storage, Self::Serde>,
    ) -> Result<()> {
        let span = trace_span!("clear_and_rebuild_state_tree");
        let _enter = span.enter();

        Self::clear_state_tree(db)?;

        let mut batch = HashMap::new();
        for (i, table) in db.tables().into_iter().enumerate() {
            let tid = i as u8;

            let mut changes = Vec::new();
            for (key, value) in db.get_storage_backend_unsafe().get_all(tid) {
                changes.push((key, Operation::Insert(value)));
            }
            batch.insert(table, changes.into_iter());
        }

        db.run(|ctx| Self::update_state_tree(ctx, batch))
    }
}
