use fxhash::{FxHashMap, FxHashSet};

use crate::frame::{Digest, MessageInternedId};

// TODO: Make this persist.
#[derive(Default)]
pub struct Database {
    data: FxHashMap<Digest, Entry>,
}

struct Entry {
    id: MessageInternedId,
}

impl Database {}
