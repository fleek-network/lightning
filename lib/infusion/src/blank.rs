use std::future::Future;
use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

/// The object that is meant to be the placeholder nullified implementer
/// of every generic trait.
#[derive(Copy, Serialize, Deserialize)]
pub struct Blank<C>(PhantomData<C>);

unsafe impl<C> Send for Blank<C> {}
unsafe impl<C> Sync for Blank<C> {}

impl<T> Default for Blank<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T> Clone for Blank<T> {
    fn clone(&self) -> Self {
        Self(PhantomData)
    }
}

impl<T> Future for Blank<T> {
    type Output = T;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        tracing::error!(
            "infusion::Blank<{}> was awaited",
            std::any::type_name::<T>()
        );

        std::task::Poll::Pending
    }
}
