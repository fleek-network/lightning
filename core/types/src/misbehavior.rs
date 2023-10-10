//! Types related to the proof of misbehavior.

use serde::{Deserialize, Serialize};

/// Placeholder
/// This is the proof presented to the slashing function that proves a node misbehaved and should be
/// slashed
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum ProofOfMisbehavior {}
