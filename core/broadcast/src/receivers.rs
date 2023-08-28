use std::marker::PhantomData;
use std::pin::Pin;

use futures::stream::FuturesUnordered;
use futures::{Future, StreamExt};
use lightning_interfaces::ReceiverInterface;

use crate::conn::PairedReceiver;
use crate::Frame;

type Fut<R> = Pin<Box<dyn Future<Output = (PairedReceiver<R>, Option<Frame>)> + Send>>;

/// This object is a responsible for managing a large set of connection pool receivers
/// all sending us messages.
pub struct Receivers<R: ReceiverInterface<Frame>> {
    pending: FuturesUnordered<Fut<R>>,
}

impl<R: ReceiverInterface<Frame>> Default for Receivers<R> {
    fn default() -> Self {
        Self {
            pending: FuturesUnordered::new(),
        }
    }
}

impl<R: ReceiverInterface<Frame>> Receivers<R> {
    /// Returns true if there is no receiver in the queue.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Push a message receiver to the queue so we can listen for what they have to say.
    #[inline(always)]
    pub fn push(&mut self, receiver: PairedReceiver<R>) {
        log::trace!("pushed {receiver:?}");
        let future = gen_fut(receiver);
        self.pending.push(future);
    }

    /// Returns the next incoming message that we need to process. This will also
    /// return back the receiver object.
    ///
    /// The caller can decide to push the receiver back to the queue depending on
    /// the connection status.
    #[inline(always)]
    pub async fn recv(&mut self) -> Option<(PairedReceiver<R>, Option<Frame>)> {
        self.pending.next().await
    }
}

/// Given a pool receiver return a future that will resolve once `r.recv()`
/// has resolved. Which can also return the receiver object back to us as well.
fn gen_fut<R: ReceiverInterface<Frame>>(mut r: PairedReceiver<R>) -> Fut<R> {
    Box::pin(async {
        let out = r.recv().await;
        log::trace!("{r:?}: {out:?}");
        (r, out)
    })
}
