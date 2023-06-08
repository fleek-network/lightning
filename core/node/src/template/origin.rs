use std::{
    pin::Pin,
    task::{Context, Poll},
};

use tokio_stream::Stream;

pub struct MyStream {}

impl Stream for MyStream {
    type Item = bytes::BytesMut;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        todo!()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}
