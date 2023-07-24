use std::any::Any;

use crate::state::with_node;

/// Pass a reference to the registered global state value of type `T` to the closure.
///
/// # Panics
///
/// If the value has not been provided during the construction of the simulator.
pub fn with_state<F, T: Any, U>(closure: F) -> U
where
    F: FnOnce(&T) -> U,
{
    with_node(|n| n.storage.with(closure))
}
