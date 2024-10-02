use serde::{Deserialize, Serialize};

use crate::Blake3Hash;

#[derive(
    Debug, Default, Hash, Clone, Serialize, Deserialize, Eq, PartialEq, schemars::JsonSchema,
)]
pub struct ContentUpdate {
    pub uri: Blake3Hash,
    pub remove: bool,
}
