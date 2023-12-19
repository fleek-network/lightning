use serde::{Deserialize, Serialize};

use crate::{Blake3Hash, NodeIndex};

#[derive(Debug, Hash, Clone, Serialize, Deserialize, Eq, PartialEq, schemars::JsonSchema)]
pub struct ContentUpdate {
    pub cid: Blake3Hash,
    pub provider: NodeIndex,
    pub remove: bool,
}
