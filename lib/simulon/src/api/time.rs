use std::time::Duration;

use crate::state::with_node;

/// Returns the current time.
pub fn now() -> u128 {
    with_node(|n| n.now())
}

pub async fn sleep(_time: Duration) {}
