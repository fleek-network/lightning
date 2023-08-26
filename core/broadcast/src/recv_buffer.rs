use smallvec::SmallVec;
use tokio::sync::oneshot;

use crate::command::{RecvCmd, SharedMessage};
use crate::ring::MessageRing;

/// A wrapper around the [`MessageRing`] which can also keep track of a set of
/// of pending oneshot sender channels.
///
/// This is where we actually 'respond' to a recv request.
pub struct RecvBuffer<T = SharedMessage> {
    ring: MessageRing<T>,
    pending: SmallVec<[oneshot::Sender<(usize, T)>; 4]>,
}

impl<T> From<MessageRing<T>> for RecvBuffer<T> {
    fn from(ring: MessageRing<T>) -> Self {
        Self {
            ring,
            pending: SmallVec::default(),
        }
    }
}

impl<T: Clone> RecvBuffer<T> {
    /// Push a new data to this buffer. Responds to any pending waiters from
    /// prior call to `respond_to`.
    pub fn insert(&mut self, data: T) {
        if self.pending.is_empty() {
            self.ring.push(data);
        } else {
            let cloned = data.clone();
            let index = self.ring.push(cloned);

            let mut next_data = None;

            while self.pending.len() > 1 {
                let chan = self.pending.pop().unwrap();
                let value = next_data.take().unwrap_or_else(|| data.clone());
                match chan.send((index, value)) {
                    Ok(_) => {},
                    Err((_, val)) => {
                        // store it for next iteration to avoid clone.
                        next_data = Some(val);
                    },
                }
            }

            let chan = self.pending.pop().unwrap();
            let _ = chan.send((index, data));
        }
    }

    /// Respond to a recv request, if there is no new data available keeps track of the channel
    /// and responds once new data is available upon insertion.
    pub fn response_to(&mut self, last_seen: Option<usize>, chan: oneshot::Sender<(usize, T)>) {
        let Some(res) = self.ring.recv(last_seen) else {
            self.pending.push(chan);
            return;
        };

        let _ = chan.send(res);
    }
}
