use std::future::Future;
use std::pin::Pin;
use std::task::Poll;

use deno_core::v8::OwnedIsolate;

use crate::runtime::Runtime;

/// Guards the Isolate of a Deno runtime instance.
///
/// This is simply a wrapper for any future that needs
/// to handle a `JsRuntime` instance. It maintains a
/// thread state invariant required by the library
/// so that multiple futures can handle multiple
/// instances in the same thread.
pub struct IsolateGuard {
    rt: Runtime,
}

impl IsolateGuard {
    pub fn new(rt: Runtime) -> Self {
        Self { rt }
    }

    pub fn guard<'a, F>(&'a mut self, f: F) -> GuardedIsolateFuture<'a>
    where
        F: FnOnce(&'a mut Runtime) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + 'a>>,
    {
        let ptr = self.rt.deno.v8_isolate() as *mut _;
        GuardedIsolateFuture {
            ptr,
            fut: f(&mut self.rt),
        }
    }

    pub fn destroy(self) -> Runtime {
        self.rt
    }
}

pub struct GuardedIsolateFuture<'a> {
    ptr: *mut OwnedIsolate,
    fut: Pin<Box<dyn Future<Output = anyhow::Result<()>> + 'a>>,
}

impl GuardedIsolateFuture<'_> {
    fn enter(&mut self) {
        let runtime = unsafe { self.ptr.as_mut().expect("Pointer to be non-null") };
        unsafe {
            runtime.enter();
        }
    }

    fn exit(&mut self) {
        let runtime = unsafe { self.ptr.as_mut().expect("Pointer to be non-null") };
        unsafe {
            runtime.exit();
        }
    }
}

impl Future for GuardedIsolateFuture<'_> {
    type Output = anyhow::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        self.as_mut().enter();
        let res = self.as_mut().fut.as_mut().poll(cx);
        self.as_mut().exit();
        res
    }
}
