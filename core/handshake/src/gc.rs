//! Implementation of the connection garbage collection.

use std::time::Duration;

use lightning_interfaces::ExecutorProviderInterface;

use crate::state::StateRef;
use crate::utils::now;

struct GcEntry {
    connection_id: u64,
    disconnected_at: u64,
}

pub struct Gc {
    rx: tokio::sync::mpsc::Receiver<GcEntry>,
    tx: tokio::sync::mpsc::Sender<GcEntry>,
}

impl Default for Gc {
    fn default() -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(500_000);
        Self { tx, rx }
    }
}

impl Gc {
    pub fn sender(&self) -> GcSender {
        GcSender {
            sender: self.tx.clone(),
        }
    }

    pub fn spawn<P: ExecutorProviderInterface>(self, state: StateRef<P>, timeout: Duration) {
        let rx = self.rx;
        tokio::spawn(gc_loop(rx, state, timeout));
    }
}

pub struct GcSender {
    sender: tokio::sync::mpsc::Sender<GcEntry>,
}

impl GcSender {
    /// Add the connection id to the gc, the connection will be
    /// garbage collected after the time has passed.
    pub async fn insert(&self, connection_id: u64) {
        let _ = self
            .sender
            .send(GcEntry {
                connection_id,
                disconnected_at: now(),
            })
            .await;
    }
}

async fn gc_loop<P: ExecutorProviderInterface>(
    mut rx: tokio::sync::mpsc::Receiver<GcEntry>,
    state: StateRef<P>,
    timeout: Duration,
) {
    const EPSILON: u64 = 500;

    let timeout: u64 = timeout.as_millis() as u64; // 2s
    let mut consumed_entry: Option<GcEntry> = None;

    // TODO(qti3e): Refactor/rewrite this. Sometimes it calls `now()` more than necessary.
    'outer: loop {
        let time = now(); // 7s
        let expired = time - timeout; // 5s

        if let Some(entry) = consumed_entry.take() {
            if entry.disconnected_at > expired {
                let wait_for = entry.disconnected_at - expired + EPSILON;
                tokio::time::sleep(Duration::from_millis(wait_for)).await;

                // Put the entry back and retry.
                consumed_entry = Some(entry);
                continue 'outer; // to recompute and refresh
            } else {
                state.gc_tick(entry.disconnected_at, entry.connection_id);
            }
        }

        while let Ok(entry) = rx.try_recv() {
            if entry.disconnected_at <= expired {
                // disconnected_at = 4s
                // --> expired
                state.gc_tick(entry.disconnected_at, entry.connection_id);
            } else {
                // disconnected_at = 5.1s, 8s
                // --> keep
                consumed_entry = Some(entry);
                continue 'outer;
            }
        }

        // await for the next event and write to the consumed entry so
        // we can check it again at the beginning of the loop.
        if let Some(entry) = rx.recv().await {
            consumed_entry = Some(entry);
        } else {
            // Nothing is coming anymore.
            break;
        }
    }
}
