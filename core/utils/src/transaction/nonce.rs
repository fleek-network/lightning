use std::sync::Arc;

use lightning_interfaces::types::Nonce;
use tokio::sync::RwLock;

#[derive(Clone)]
pub(crate) struct NonceState {
    inner: Arc<RwLock<NonceStateInner>>,
}

impl NonceState {
    pub fn new(base: Nonce) -> Self {
        Self {
            inner: Arc::new(RwLock::new(NonceStateInner {
                base,
                next: base + 1,
            })),
        }
    }

    pub async fn update(&self, base: Nonce) {
        let mut inner = self.inner.write().await;
        inner.base = base;
        inner.next = base + 1;
    }

    pub async fn get_next_and_increment(&self) -> Nonce {
        let mut inner = self.inner.write().await;
        let next = inner.next;
        inner.next += 1;
        next
    }

    pub async fn get_next(&self) -> Nonce {
        let inner = self.inner.read().await;
        inner.next
    }

    pub async fn get_base(&self) -> Nonce {
        let inner = self.inner.read().await;
        inner.base
    }
}

pub(crate) struct NonceStateInner {
    base: Nonce,
    next: Nonce,
}
