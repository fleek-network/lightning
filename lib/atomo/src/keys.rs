use crate::batch::BoxedVec;

pub type ImKeyCollection = im::HashSet<BoxedVec, fxhash::FxBuildHasher>;

pub type MaybeImKeyCollection = Option<ImKeyCollection>;

/// Analogues to the [`VerticalBatch`] this data structure is responsible for
/// holding an immutable data containing all of the keys for each table, given
/// that the iterator functionality is enabled for the table.
#[derive(Default)]
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
}
