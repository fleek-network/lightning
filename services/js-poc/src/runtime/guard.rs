use std::future::Future;
use std::pin::Pin;
use std::task::Poll;

use deno_core::v8::OwnedIsolate;

use crate::runtime::Runtime;

pub struct IsolateGuard<'a> {
    ptr: *mut OwnedIsolate,
    f: Pin<Box<dyn Future<Output = anyhow::Result<()>> + 'a>>,
}

impl<'a> IsolateGuard<'a> {
    pub fn new<F>(mut runtime: &'a mut Runtime, f: F) -> Self
    where
        F: FnOnce(&'a mut Runtime) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + 'a>>,
    {
        let isolate = runtime.deno.v8_isolate() as *mut _;
        Self {
            ptr: isolate,
            f: f(runtime),
        }
    }

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

impl<'a> Future for IsolateGuard<'a> {
    type Output = anyhow::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        self.as_mut().enter();
        let res = self.as_mut().f.as_mut().poll(cx);
        self.as_mut().exit();
        res
    }
}
