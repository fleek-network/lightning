use derive_more::IsVariant;
use serde::{Deserialize, Serialize};

/// The protocol's pre-defined connection scopes.
// Feel free to add more variant to the end on a need-to-have basis.
#[derive(
    Debug, Clone, Copy, PartialEq, PartialOrd, Ord, Eq, Hash, Serialize, Deserialize, IsVariant,
)]
#[repr(u8)]
pub enum ServiceScope {
    Dht = 0x00,
    Broadcast = 0x01,
    Debug = 0xFF,
}
