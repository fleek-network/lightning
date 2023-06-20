use std::{future::Future, marker::PhantomData};

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

pub struct RecvFuture<T> {
    p: PhantomData<T>,
}

impl<T> Future for RecvFuture<T> {
    type Output = Option<T>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        todo!()
    }
}
