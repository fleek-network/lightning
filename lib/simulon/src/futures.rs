use std::{future::Future, time::Duration};

use crate::connection::Connection;

pub struct DialFuture {}

impl Future for DialFuture {
    type Output = Option<Connection>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        todo!()
    }
}

pub struct AcceptFuture {}

impl Future for AcceptFuture {
    type Output = Option<Connection>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        todo!()
    }
}

pub struct SleepFuture {}

impl Future for SleepFuture {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        todo!()
    }
}

#[derive(Default)]
pub struct RecvFuture {
    is_ready: bool,
}

impl Future for RecvFuture {
    type Output = Option<Vec<u8>>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        println!("poll {}", self.is_ready);

        if !self.is_ready {
            self.is_ready = true;

            let waker = cx.waker().clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::new(5, 0));
                println!("after sleep");
                waker.wake();
            });

            std::task::Poll::Pending
        } else {
            std::task::Poll::Ready(Some(vec![0, 1]))
        }
    }
}

#[test]
fn x() {
    let future = async {
        println!("A");

        let x = RecvFuture::default().await;

        println!("{x:?}");
    };

    futures::executor::block_on(future);
}
