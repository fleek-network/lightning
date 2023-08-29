use crate::batch::BoxedVec;
use crate::db::TableId;

pub type ImKeyCollection = im::OrdSet<BoxedVec>;

pub type MaybeImKeyCollection = Option<ImKeyCollection>;

/// Analogues to the [`VerticalBatch`] this data structure is responsible for
/// holding an immutable data containing all of the keys for each table, given
/// that the iterator functionality is enabled for the table.
#[derive(Default, Clone)]
pub struct VerticalKeys(Vec<MaybeImKeyCollection>);

impl VerticalKeys {
    /// Enable the indexer on the given table index.
    pub fn enable(&mut self, index: usize) {
        if index < self.0.len() {
            self.0[index] = Some(ImKeyCollection::default());
        } else {
            self.0.resize(index, None);
            self.0.push(Some(ImKeyCollection::default()));
        }
    }

    #[inline(always)]
    pub fn update<F>(&mut self, tid: TableId, closure: F)
    where
        F: FnOnce(&mut ImKeyCollection),
    {
        let tid = tid as usize;
        if tid < self.0.len() {
            if let Some(data) = &mut self.0[tid] {
                closure(data);
            }
        }
    }

    #[inline(always)]
    pub fn get(&self, tid: TableId) -> &MaybeImKeyCollection {
        let tid = tid as usize;
        if tid >= self.0.len() {
            return &None;
        }
        &self.0[tid]
    }
}
