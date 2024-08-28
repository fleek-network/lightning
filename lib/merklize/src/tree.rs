use std::collections::HashMap;

use anyhow::Result;
use atomo::batch::Operation;
use atomo::{
    Atomo,
    AtomoBuilder,
    QueryPerm,
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

/// A trait for maintaining and interacting with a state tree.
///
/// ## Examples
///
/// ```rust
#[doc = include_str!("../examples/jmt-sha256.rs")]
/// ```
pub trait StateTree {
    type Storage: StorageBackend;
    type Serde: SerdeBackend;
    type Hasher: SimpleHasher;
    type Proof: StateProof;

    /// Registers the necessary tables for the state tree on the given atomo builder.
    fn register_tables<C: StorageBackendConstructor>(
        builder: AtomoBuilder<C, Self::Serde>,
    ) -> AtomoBuilder<C, Self::Serde>;

    /// Returns the root hash of the state tree.
    fn get_state_root(ctx: &TableSelector<Self::Storage, Self::Serde>) -> Result<StateRootHash>;

    /// Generates and returns a merkle proof for the given key in the state.
    fn get_state_proof(
        ctx: &TableSelector<Self::Storage, Self::Serde>,
        table: &str,
        serialized_key: Vec<u8>,
    ) -> Result<Self::Proof>;

    /// Applies the changes in the given batch of updates to the state tree.
    ///
    /// This method uses an atomo execution context, so it is safe to use concurrently.
    fn update_state_tree<I>(
        ctx: &TableSelector<Self::Storage, Self::Serde>,
        batch: HashMap<String, I>,
    ) -> Result<()>
    where
        I: Iterator<Item = (Box<[u8]>, Operation)>;

    /// Clears the existing state tree data. This does not delete or modify any of the state data,
    /// just the tree structure and tables related to it.
    ///
    /// This method is suffixed as unsafe because it reads and writes directly to the storage
    /// backend, bypassing the normal atomo update and batching mechanisms. It should only be used
    /// with caution in isolation for use cases such as performing backfills or recovering from
    /// corruption.
    fn clear_state_tree_unsafe(
        db: &mut Atomo<UpdatePerm, Self::Storage, Self::Serde>,
    ) -> Result<()>;

    /// Verifies that the state in the given atomo database instance, when used to build a
    /// new, temporary state tree from scratch, matches the stored state tree root hash.
    ///
    /// This method is suffixed as unsafe because it reads directly from the storage backend,
    /// bypassing the normal atomo update and batching mechanisms. It should only be used with
    /// caution in isolation for use cases such as performing an integrity check at startup.
    fn verify_state_tree_unsafe(
        db: &mut Atomo<QueryPerm, Self::Storage, Self::Serde>,
    ) -> Result<()>;

    /// Applies the pending changes in the given context to the state tree.
    ///
    /// This is an implementation that makes use of the `update_state_tree` method, passing it the
    /// batch of pending changes from the context.
    fn update_state_tree_from_context_changes(
        ctx: &TableSelector<Self::Storage, Self::Serde>,
    ) -> Result<()> {
        let span = trace_span!("update_state_tree_from_context_changes");
        let _enter = span.enter();

        let mut table_name_by_id = FxHashMap::default();
        for (i, table) in ctx.tables().into_iter().enumerate() {
            table_name_by_id.insert(i as TableId, table);
        }

        // Build batch of pending changes from the context.
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
    /// This method is suffixed as unsafe because it reads and writes directly to the storage
    /// backend, bypassing the normal atomo update and batching mechanisms. It should only be used
    /// with caution in isolation for use cases such as performing backfills or recovering from
    /// corruption.
    fn clear_and_rebuild_state_tree_unsafe(
        db: &mut Atomo<UpdatePerm, Self::Storage, Self::Serde>,
    ) -> Result<()> {
        let span = trace_span!("clear_and_rebuild_state_tree_unsafe");
        let _enter = span.enter();

        Self::clear_state_tree_unsafe(db)?;

        // Build batch of all state data.
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

    /// Returns true if the state tree is empty, and false otherwise.
    ///
    /// This method is suffixed as unsafe because it reads directly from the storage backend,
    /// bypassing the normal atomo update and batching mechanisms. It should only be used with
    /// caution in isolation for use cases such as performing checks at startup.
    fn is_empty_state_tree_unsafe(
        db: &mut Atomo<QueryPerm, Self::Storage, Self::Serde>,
    ) -> Result<bool>;
}
