use std::future::Future;

/// A waiter that can be notified when a state changes.
///
/// This is a single-use notification. After the first notification, the waiter is
/// considered ready and any further notifications are ignored.
pub trait ReadyWaiter: Default + Clone + Send + Sync {
    type State: ReadyWaiterState;

    /// Notify ready and provide the state.
    ///
    /// This should notify all consumers that are currently waiting for the state to
    /// change via `wait`.
    ///
    /// Calling this multiple times should have no effect after the first call.
    fn notify(&self, state: Self::State);

    /// Wait for the state to change.
    ///
    /// This should return immediately if the state is already ready.
    fn wait(&self) -> impl Future<Output = Self::State> + Send;

    /// Check if the state is ready.
    fn is_ready(&self) -> bool;

    /// Get the current state.
    ///
    /// This should return `None` if not yet ready.
    fn state(&self) -> Option<Self::State>;
}

pub trait ReadyWaiterState: Default + Clone + Send + Sync {}

impl<T: Default + Clone + Send + Sync> ReadyWaiterState for T {}
