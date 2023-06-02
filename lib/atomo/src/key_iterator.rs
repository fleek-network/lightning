use std::marker::PhantomData;

use fxhash::FxHashSet;

use crate::{batch::BoxedVec, db::TableId};

/// An iterator over the keys of the table.
pub struct KeyIterator<K> {
    _tid: TableId,
    _removed: FxHashSet<BoxedVec>,
    _inserted: FxHashSet<BoxedVec>,
    _key: PhantomData<K>,
}
