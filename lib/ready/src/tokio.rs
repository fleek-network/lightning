use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::{ReadyWaiter, ReadyWaiterState};

/// A ready waiter that uses a [tokio::sync::Notify] internally to signal when a state changes.
///
/// This is a single-use notification. After the first notification, the waiter is
/// considered ready and any further notifications are ignored.
#[derive(Clone)]
pub struct TokioReadyWaiter<T: ReadyWaiterState> {
    notify: Arc<tokio::sync::Notify>,
    state: Arc<Mutex<Option<T>>>,
    is_ready: Arc<AtomicBool>,
}

impl<T: ReadyWaiterState> TokioReadyWaiter<T> {
    pub fn new() -> Self {
        Self {
            notify: Arc::new(tokio::sync::Notify::new()),
            state: Arc::new(Mutex::new(None)),
            is_ready: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl<T: ReadyWaiterState> ReadyWaiter for TokioReadyWaiter<T> {
    type State = T;

    /// Notify ready and provide the state.
    fn notify(&self, state: T) {
        if self.is_ready() {
            return;
        }
        *self.state.lock().unwrap() = Some(state);
        self.is_ready.store(true, Ordering::Relaxed);
        self.notify.notify_waiters();
    }

    /// Wait for the state to change.
    async fn wait(&self) -> T {
        if self.is_ready() {
            return self.state().clone().expect("state was not set");
        }
        self.notify.notified().await;
        self.state
            .lock()
            .unwrap()
            .clone()
            .expect("state was not set")
    }

    /// Check if the state is ready.
    fn is_ready(&self) -> bool {
        self.is_ready.load(Ordering::Relaxed)
    }

    /// Get the current state.
    fn state(&self) -> Option<T> {
        self.state.lock().unwrap().clone()
    }
}

impl<T: ReadyWaiterState> Default for TokioReadyWaiter<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    struct TestReadyState {
        pub ready: bool,
    }

    #[tokio::test]
    async fn test_ready_waiter_notify_single() {
        let ready = TokioReadyWaiter::new();

        // Should not be ready initially.
        assert!(!ready.is_ready());
        assert!(ready.state().is_none());

        // Should be ready after notification.
        let state = TestReadyState { ready: true };
        ready.notify(state.clone());
        assert!(ready.wait().await.ready);
        assert!(ready.is_ready());
        assert_eq!(ready.state(), Some(state.clone()));

        // Should return immediately if already ready.
        assert_eq!(ready.wait().await, state);
    }

    #[tokio::test]
    async fn test_ready_waiter_notify_many() {
        let ready = TokioReadyWaiter::new();
        let ready1 = ready.clone();
        let ready2 = ready.clone();
        let ready3 = ready.clone();

        // Should not be ready initially.
        assert!(!ready.is_ready());
        assert!(ready.state().is_none());
        assert!(!ready1.is_ready());
        assert!(ready1.state().is_none());
        assert!(!ready2.is_ready());
        assert!(ready2.state().is_none());
        assert!(!ready3.is_ready());
        assert!(ready3.state().is_none());

        // Should be ready after notification.
        let state = TestReadyState { ready: true };
        ready.notify(state.clone());

        assert!(ready.wait().await.ready);
        assert!(ready.is_ready());
        assert_eq!(ready.state(), Some(state.clone()));

        assert!(ready1.wait().await.ready);
        assert!(ready1.is_ready());
        assert_eq!(ready1.state(), Some(state.clone()));

        assert!(ready2.wait().await.ready);
        assert!(ready2.is_ready());
        assert_eq!(ready2.state(), Some(state.clone()));

        assert!(ready3.wait().await.ready);
        assert!(ready3.is_ready());
        assert_eq!(ready3.state(), Some(state.clone()));

        // Should return immediately if already ready.
        assert_eq!(ready.wait().await, state);
        assert_eq!(ready1.wait().await, state);
        assert_eq!(ready2.wait().await, state);
        assert_eq!(ready3.wait().await, state);
    }
}
