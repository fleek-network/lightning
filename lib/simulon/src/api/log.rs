use crate::state::with_node;

/// Emit an event from the node.
///
/// # Panics
///
/// If the event has already been emitted on the same node.
pub fn emit(key: impl Into<String>) {
    with_node(|n| n.emit(key.into()));
}
