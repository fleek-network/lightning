use crate::command::SharedMessage;

/// The message ring is where incoming messages come and sit until a `PubSub::recv` tries
/// to poll them.
///
/// It behaves like a fixed capacity ring buffer, if there is no ::recv call trying to poll
/// messages after the capacity is hit, old messages are overwritten.
///
/// However it can still provide us with a mechanism for:
///
/// 1. Handling when we get messages before any pubsub is ready to claim those.
/// 2. Achieving broadcast like functionality when multiple PubSub's exits in the same topic, we
///    don't want one of those to lose messages.
pub struct MessageRing<T> {
    next_index: usize,
    capacity: usize,
    buffer: Vec<T>,
}

impl<T> MessageRing<T> {
    /// Create a new message ring with the given capacity.
    pub fn new(capacity: usize) -> Self {
        let cap = if capacity == 0 { usize::MAX } else { capacity };

        Self {
            next_index: 0,
            capacity: cap,
            buffer: Vec::with_capacity(capacity),
        }
    }

    /// Push a message to the back of this list.
    pub fn push(&mut self, message: T) -> usize {
        let index = self.next_index;
        self.next_index += 1;

        if self.buffer.len() < self.capacity {
            self.buffer.push(message);
        } else {
            let pos = index % self.capacity;
            self.buffer[pos] = message;
        }

        index
    }

    /// Return the last message a receiver has to observe depending on what they have seen
    /// last.
    ///
    /// # Returns
    ///
    /// The oldest unseen message along with its id.
    pub fn recv(&self, last_seen: Option<usize>) -> Option<(usize, T)>
    where
        T: Clone,
    {
        // cap = 5
        // 0   1   2   3   4 ; fill the buffer
        // 5   1   2   3   4 ; push 5
        // 5   6   2   3   4 ; push 6
        // 5   6   7   3   4 ; push 7
        // 5   6   7   8   4 ; push 8
        // 5   6   7   8   9 ; push 9
        // 10  6   7   8   9 ; push 10
        // 10  11  7   8   9 ; push 11
        //      next index = 12
        //      last seen  = 1
        //      ; We compute the desired index that the caller wants. That is always
        //      ; just +1 of what they have seen before. Or `0` if they have never seen
        //      ; anything before.
        let desired = last_seen.map(|n| n + 1).unwrap_or(0);
        if desired >= self.next_index {
            debug_assert_eq!(
                desired, self.next_index,
                "index {last_seen:?} was never given out."
            );
            return None;
        }
        //      ; we find the index of the oldest thing we still have.
        //      ; In this state that's 7. Generally:
        let begin = self.next_index.saturating_sub(self.capacity);
        //      ; What we can give the caller only starts at `begin`, let's lets raise
        //      ; what they want to that number if the actual index is smaller.
        let index = desired.max(begin);
        let pos = index % self.capacity;
        Some((index, self.buffer[pos].clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let mut ring = MessageRing::<usize>::new(5);
        for i in 0..12 {
            ring.push(i);
        }
        assert_eq!(ring.recv(None), Some((7, 7)));
        assert_eq!(ring.recv(Some(0)), Some((7, 7)));
        assert_eq!(ring.recv(Some(5)), Some((7, 7)));
        assert_eq!(ring.recv(Some(6)), Some((7, 7)));
        assert_eq!(ring.recv(Some(7)), Some((8, 8)));
        assert_eq!(ring.recv(Some(8)), Some((9, 9)));
        assert_eq!(ring.recv(Some(9)), Some((10, 10)));
        assert_eq!(ring.recv(Some(10)), Some((11, 11)));
        assert_eq!(ring.recv(Some(11)), None);
    }
}
