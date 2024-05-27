use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::task::{Poll, Waker};

use triomphe::Arc;

use crate::ipc_types::Response;

#[derive(Default)]
struct RequestFutureState {
    responded_to: bool,
    response: Option<Response>,
    waker: Option<Waker>,
}

#[derive(Clone, Default)]
struct StateContainer(Arc<Mutex<RequestFutureState>>);

impl StateContainer {
    #[inline(always)]
    pub fn into_raw(self) -> RequestCtx {
        RequestCtx(Arc::into_raw(self.0) as *const ())
    }

    #[inline(always)]
    pub fn from_raw(raw: RequestCtx) -> Self {
        unsafe { Self(Arc::from_raw(raw.0 as *const _)) }
    }
}

pub struct RequestFuture {
    state: StateContainer,
}

unsafe impl Send for RequestFuture {}
unsafe impl Sync for RequestFuture {}

impl Future for RequestFuture {
    type Output = Response;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let self_ref = Pin::into_ref(self);
        let mut state = self_ref.state.0.lock().expect("Poisoned future lock");

        if let Some(response) = state.response.take() {
            Poll::Ready(response)
        } else {
            assert!(!state.responded_to, "poll after receive");
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

#[inline(always)]
pub(crate) fn create_future() -> (RequestCtx, RequestFuture) {
    let state = StateContainer::default();
    let raw = state.clone().into_raw();
    (raw, RequestFuture { state })
}

#[inline(always)]
pub(crate) fn future_callback(ctx: RequestCtx, response: Response) {
    let state = StateContainer::from_raw(ctx);
    let mut state = state.0.lock().expect("Poisoned future lock");
    assert!(!state.responded_to, "already responded to future.");
    state.response = Some(response);
    state.responded_to = true;
    if let Some(w) = state.waker.take() {
        w.wake();
    }
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct RequestCtx(*const ());
unsafe impl Send for RequestCtx {}
unsafe impl Sync for RequestCtx {}
impl From<RequestCtx> for u64 {
    #[inline(always)]
    fn from(value: RequestCtx) -> Self {
        value.0 as usize as u64
    }
}
impl From<u64> for RequestCtx {
    #[inline(always)]
    fn from(value: u64) -> Self {
        Self(value as usize as *const ())
    }
}
