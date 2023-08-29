use std::cell::UnsafeCell;
use std::future::Future;
use std::pin::Pin;
use std::task::{Poll, Waker};

use triomphe::Arc;

use crate::internal::Response;

#[derive(Default)]
struct RequestFutureState {
    responded_to: bool,
    response: Option<Response>,
    waker: Option<Waker>,
}

#[derive(Clone, Default)]
struct StateContainer(Arc<UnsafeCell<RequestFutureState>>);

impl StateContainer {
    pub fn into_raw(self) -> RequestCtx {
        RequestCtx(Arc::into_raw(self.0) as *const ())
    }

    pub fn from_raw(raw: RequestCtx) -> Self {
        unsafe { Self(Arc::from_raw(raw.0 as *const _)) }
    }

    pub fn as_mut(&self) -> *mut RequestFutureState {
        self.0.get()
    }
}

pub struct RequestFuture {
    state: StateContainer,
}

impl Future for RequestFuture {
    type Output = Response;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let self_ref = Pin::into_ref(self);
        let state = unsafe { &mut *self_ref.state.as_mut() };
        if let Some(response) = state.response.take() {
            Poll::Ready(response)
        } else {
            assert!(!state.responded_to, "poll after receive");
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

pub(crate) fn create_future() -> (RequestCtx, RequestFuture) {
    let state = StateContainer::default();
    let raw = state.clone().into_raw();
    (raw, RequestFuture { state })
}

pub(crate) fn future_callback(ctx: RequestCtx, response: Response) {
    let state = StateContainer::from_raw(ctx);
    let state_mut = unsafe { &mut *state.as_mut() };
    assert!(!state_mut.responded_to, "already responded to future.");
    state_mut.response = Some(response);
    state_mut.responded_to = true;
    if let Some(w) = state_mut.waker.take() {
        w.wake();
    }
}

#[derive(Copy, Clone, Debug)]
pub struct RequestCtx(*const ());
unsafe impl Send for RequestCtx {}
unsafe impl Sync for RequestCtx {}
