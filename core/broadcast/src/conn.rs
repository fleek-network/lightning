use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use fleek_crypto::NodePublicKey;
use lightning_interfaces::{ReceiverInterface, SenderInterface};
use tokio::sync::oneshot;

use crate::Frame;

/// Given a pair of sender and receiver, returns a wrapped version of the inputs. Once the
/// new sender is dropped the pending recv call will return None and the receiver will be
/// dropped.
pub fn create_pair<S: SenderInterface<Frame>, R: ReceiverInterface<Frame>>(
    sender: S,
    receiver: R,
) -> (PairedSender<S>, PairedReceiver<R>) {
    static TAG: AtomicUsize = AtomicUsize::new(0);
    let tag = TAG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let (cancel_tx, cancel_rx) = oneshot::channel();
    (
        PairedSender {
            inner: Arc::new(PairedSenderInner {
                tag,
                cancel_tx: Some(cancel_tx),
                sender,
            }),
        },
        PairedReceiver {
            tag,
            inner: PairedReceiverInner::NotDropped {
                cancel_rx,
                receiver,
            },
        },
    )
}

/// A generic wrapper around connection pool sender which cancels the listener on drop.
pub struct PairedSender<S: SenderInterface<Frame>> {
    inner: Arc<PairedSenderInner<S>>,
}

struct PairedSenderInner<S> {
    tag: usize,
    cancel_tx: Option<oneshot::Sender<()>>,
    sender: S,
}

pub struct PairedReceiver<R: ReceiverInterface<Frame>> {
    tag: usize,
    inner: PairedReceiverInner<R>,
}

enum PairedReceiverInner<R> {
    Dropped {
        pk: NodePublicKey,
    },
    NotDropped {
        cancel_rx: oneshot::Receiver<()>,
        receiver: R,
    },
}

impl<R: ReceiverInterface<Frame>> PairedReceiver<R> {
    /// Returns true if receiver and sender are meant to be a pair.
    pub fn is_receiver_of<S: SenderInterface<Frame>>(&self, sender: &PairedSender<S>) -> bool {
        self.tag == sender.inner.tag
    }

    pub fn pk(&self) -> &NodePublicKey {
        match &self.inner {
            PairedReceiverInner::Dropped { pk } => pk,
            PairedReceiverInner::NotDropped { receiver, .. } => receiver.pk(),
        }
    }

    #[inline(always)]
    pub async fn recv(&mut self) -> Option<Frame> {
        let PairedReceiverInner::NotDropped { cancel_rx, receiver, } = &mut self.inner else {
            return None;
        };

        tokio::select! {
            _ = cancel_rx => {
                let pk = *receiver.pk();
                self.inner = PairedReceiverInner::Dropped { pk };
                None
            },
            value = receiver.recv() => {
                value
            }
        }
    }
}

impl<S> Drop for PairedSenderInner<S> {
    fn drop(&mut self) {
        let _ = self.cancel_tx.take().unwrap().send(());
    }
}

impl<S: SenderInterface<Frame>> Clone for PairedSender<S> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<S: SenderInterface<Frame>> Deref for PairedSender<S> {
    type Target = S;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.inner.sender
    }
}
