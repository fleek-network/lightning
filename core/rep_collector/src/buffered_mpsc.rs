use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use tokio::sync::mpsc::{channel, error::SendError, Receiver, Sender};

pub struct BufferedSender<T> {
    inner: Sender<Vec<T>>,
    // The buffer is wrapped in a mutex for interior mutability.
    // Each new sender clone receives its own buffer, to no one
    // will ever have to wait to acquire the mutex guard.
    buffer: Arc<Mutex<Vec<T>>>,
    max_buffer_size: usize,
}

impl<T> BufferedSender<T> {
    pub fn new(inner: Sender<Vec<T>>, max_buffer_size: usize) -> Self {
        Self {
            inner,
            buffer: Arc::new(Mutex::new(Vec::with_capacity(max_buffer_size))),
            max_buffer_size,
        }
    }

    pub async fn send(&self, value: T) -> Result<(), SendError<Vec<T>>> {
        self.push_to_buffer(value);
        if self.get_buffer_size() >= self.max_buffer_size {
            let buffer = self.get_buffer();
            self.inner.send(buffer).await
        } else {
            Ok(())
        }
    }

    pub async fn flush(&self) -> Result<(), SendError<Vec<T>>> {
        let buffer = self.get_buffer();
        self.inner.send(buffer).await
    }

    fn push_to_buffer(&self, value: T) {
        self.buffer.lock().unwrap().push(value);
    }

    fn get_buffer(&self) -> Vec<T> {
        let mut buffer = self.buffer.lock().unwrap();
        std::mem::replace(&mut *buffer, Vec::with_capacity(self.max_buffer_size))
    }

    fn get_buffer_size(&self) -> usize {
        self.buffer.lock().unwrap().len()
    }
}

impl<T> Clone for BufferedSender<T> {
    fn clone(&self) -> Self {
        BufferedSender {
            inner: self.inner.clone(),
            buffer: Arc::new(Mutex::new(Vec::with_capacity(self.max_buffer_size))),
            max_buffer_size: self.max_buffer_size,
        }
    }
}

pub struct BufferedReceiver<T> {
    inner: Receiver<Vec<T>>,
    buffer: VecDeque<T>,
}

impl<T> BufferedReceiver<T> {
    pub fn new(inner: Receiver<Vec<T>>) -> Self {
        Self {
            inner,
            buffer: VecDeque::new(),
        }
    }

    pub async fn recv(&mut self) -> Option<T> {
        let inner_closed = match self.inner.recv().await {
            Some(buffer) => {
                self.buffer.extend(buffer);
                false
            },
            None => true,
        };
        if self.buffer.is_empty() && inner_closed {
            return None;
        }
        self.buffer.pop_front()
    }
}

pub fn buffered_channel<T>(
    max_buffer_size: usize,
    channel_buffer: usize,
) -> (BufferedSender<T>, BufferedReceiver<T>) {
    let (inner_tx, inner_rx) = channel(channel_buffer);
    (
        BufferedSender::new(inner_tx, max_buffer_size),
        BufferedReceiver::new(inner_rx),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_send_and_recv() -> anyhow::Result<()> {
        let (tx, mut rx) = buffered_channel(10, 2048);

        tokio::spawn(async move {
            for i in 0..100 {
                tx.send(i).await.unwrap();
            }
        });

        for target in 0..100 {
            let i = rx.recv().await.unwrap();
            assert_eq!(i, target);
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_flush() -> anyhow::Result<()> {
        let (tx, mut rx) = buffered_channel(10, 2048);

        tokio::spawn(async move {
            for i in 0..105 {
                tx.send(i).await.unwrap();
            }
            tx.flush().await.unwrap();
        });

        for target in 0..105 {
            let i = rx.recv().await.unwrap();
            assert_eq!(i, target);
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_sender_dropped() -> anyhow::Result<()> {
        let (tx, mut rx) = buffered_channel::<i32>(10, 2048);

        tokio::spawn(async move {
            drop(tx);
        });

        assert_eq!(rx.recv().await, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_one_sender_dropped() -> anyhow::Result<()> {
        let (tx1, mut rx) = buffered_channel(10, 2048);
        let tx2 = tx1.clone();

        tokio::spawn(async move {
            drop(tx1);
        });

        tokio::spawn(async move {
            tx2.send(42).await.unwrap();
            tx2.flush().await.unwrap();
        });

        assert_eq!(rx.recv().await, Some(42));
        Ok(())
    }

    #[tokio::test]
    async fn test_all_senders_dropped() -> anyhow::Result<()> {
        let (tx1, mut rx) = buffered_channel::<i32>(10, 100);
        let tx2 = tx1.clone();

        tokio::spawn(async move {
            drop(tx1);
        });

        tokio::spawn(async move {
            drop(tx2);
        });

        assert_eq!(rx.recv().await, None);
        Ok(())
    }
}
