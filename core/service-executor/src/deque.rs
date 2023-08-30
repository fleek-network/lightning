//! Implementation of functionality related to the connection work queue that we give
//! to `handshake`.

use std::future::Future;
use std::pin::Pin;
use std::task::Poll;

use lightning_interfaces::{ConnectionWork, ConnectionWorkStealer};
use tokio::pin;

#[derive(Clone)]
pub struct WorkScheduler {
    sender: async_channel::Sender<ConnectionWork>,
}

#[derive(Clone)]
pub struct CommandStealer {
    receiver: async_channel::Receiver<ConnectionWork>,
}

impl WorkScheduler {
    pub async fn put(&self, work: ConnectionWork) {
        self.sender
            .send(work)
            .await
            .expect("could not send through the channel.");
    }
}

impl ConnectionWorkStealer for CommandStealer {
    type AsyncFuture<'a> = StealerFuture<'a>;

    fn next(&mut self) -> Self::AsyncFuture<'_> {
        StealerFuture {
            fut: self.receiver.recv(),
        }
    }

    fn next_blocking(&mut self) -> Option<ConnectionWork> {
        self.receiver.recv_blocking().ok()
    }
}

pub struct StealerFuture<'a> {
    fut: async_channel::Recv<'a, ConnectionWork>,
}

impl<'a> Future for StealerFuture<'a> {
    type Output = Option<ConnectionWork>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let fut = &mut self.fut;
        pin!(fut);
        match fut.as_mut().poll(cx) {
            Poll::Ready(value) => Poll::Ready(value.ok()),
            Poll::Pending => Poll::Pending,
        }
    }
}
