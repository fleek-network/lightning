use crate::state::with_node;

/// Emit an event from the node. This is part of the metric API and allows you to collect
/// information about when the nodes reach a certain point in the simulation, hence using
/// the same key is not supported.
///
/// # Panics
///
/// If the event has already been emitted on the same node.
pub fn emit(key: impl Into<String>) {
    with_node(|n| n.emit(key.into()));
}
