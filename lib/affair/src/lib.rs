//! A simple tokio-based library to spawn a worker and communicate with it.
//!
//! ```
//! use affair::*;
//!
//! #[derive(Default)]
//! struct CounterWorker {
//!     current: u64,
//! }
//!
//! impl Worker for CounterWorker {
//!     type Request = u64;
//!     type Response = u64;
//!
//!     fn handle(&mut self, req: Self::Request) -> Self::Response {
//!         self.current += req;
//!         self.current
//!     }
//! }
//!
//! #[tokio::main(flavor = "current_thread")]
//! async fn main() {
//!     let socket = CounterWorker::default().spawn();
//!     assert_eq!(socket.run(10).await.unwrap(), 10);
//!     assert_eq!(socket.run(3).await.unwrap(), 13);
//! }
//! ```

use std::fmt::Display;

use futures::future::BoxFuture;
use futures::stream::FuturesUnordered;
use futures::{Future, StreamExt};
use tokio::sync::{mpsc, oneshot};

/// A non-awaiting worker that processes requests and returns a response.
pub trait Worker: Send + 'static {
    /// The payload of the request that can be sent to the worker.
    type Request: Send + 'static;

    /// The response for a request.
    type Response: Send + 'static;

    /// Implement your custom handler for this. If you need this to be an async function
    /// consider using [`AsyncWorker`] instead.
    fn handle(&mut self, req: Self::Request) -> Self::Response;

    /// Spawn the given current worker on a tokio task.
    fn spawn(self) -> Socket<Self::Request, Self::Response>
    where
        Self: Sized,
    {
        let (tx, rx) = mpsc::channel(64);
        let fut = run_non_blocking(rx, self);

        #[cfg(not(tokio_unstable))]
        tokio::spawn(fut);

        #[cfg(tokio_unstable)]
        tokio::task::Builder::new()
            .name(&format!(
                "{}: non-blocking worker",
                std::any::type_name::<Self>()
            ))
            .spawn(fut)
            .expect("failed to spawn worker");

        Socket { sender: tx }
    }

    /// Spawn the current worker with a custom task spawner.
    fn spawn_with_spawner<S, Out>(self, spawner: S) -> (Out, Socket<Self::Request, Self::Response>)
    where
        Self: Sized,
        S: FnOnce(BoxFuture<'static, ()>) -> Out,
    {
        let (tx, rx) = mpsc::channel(64);
        let future = run_non_blocking(rx, self);
        let out = spawner(Box::pin(async move {
            future.await;
        }));
        let socket = Socket { sender: tx };
        (out, socket)
    }
}

/// An awaiting worker that processes requests and returns a response.
pub trait AsyncWorker: Send + 'static {
    /// The payload of the request that can be sent to the worker.
    type Request: Send + 'static;

    /// The response for a request.
    type Response: Send + 'static;

    /// Implement your custom handler for this async worker and enjoy using `await`.
    fn handle(&mut self, req: Self::Request) -> impl Future<Output = Self::Response> + Send;

    /// Spawn the given current worker on a tokio task.
    fn spawn(self) -> Socket<Self::Request, Self::Response>
    where
        Self: Sized,
    {
        let (tx, rx) = mpsc::channel(64);
        let fut = run_non_blocking_async(rx, self);

        #[cfg(not(tokio_unstable))]
        tokio::spawn(fut);

        #[cfg(tokio_unstable)]
        tokio::task::Builder::new()
            .name(&format!(
                "{}: non-blocking async worker",
                std::any::type_name::<Self>()
            ))
            .spawn(fut)
            .expect("failed to spawn worker");

        Socket { sender: tx }
    }

    /// Spawn the current worker with a custom task spawner.
    fn spawn_with_spawner<S, Out>(self, spawner: S) -> (Out, Socket<Self::Request, Self::Response>)
    where
        Self: Sized,
        S: FnOnce(BoxFuture<'static, ()>) -> Out,
    {
        let (tx, rx) = mpsc::channel(64);
        let future = run_non_blocking_async(rx, self);
        let out = spawner(Box::pin(async move {
            future.await;
        }));
        let socket = Socket { sender: tx };
        (out, socket)
    }
}

/// An awaiting worker that processes requests and returns a response in an unordered way.
pub trait AsyncWorkerUnordered: Send + Sync + 'static {
    /// The payload of the request that can be sent to the worker.
    type Request: Send + 'static;

    /// The response for a request.
    type Response: Send + 'static;

    /// Implement your custom handler for this async worker and enjoy using `await`.
    fn handle(&self, req: Self::Request) -> impl Future<Output = Self::Response> + Send;

    /// Spawn the given current worker on a tokio task.
    fn spawn(self) -> Socket<Self::Request, Self::Response>
    where
        Self: Sized,
    {
        let (tx, rx) = mpsc::channel(64);
        let fut = async move { run_unordered_async(rx, &self).await };

        #[cfg(not(tokio_unstable))]
        tokio::spawn(fut);

        #[cfg(tokio_unstable)]
        tokio::task::Builder::new()
            .name(&format!(
                "{}: unordered async worker",
                std::any::type_name::<Self>()
            ))
            .spawn(fut)
            .expect("failed to spawn worker");

        Socket { sender: tx }
    }

    /// Spawn the current worker with a custom task spawner.
    fn spawn_with_spawner<S, Out>(self, spawner: S) -> (Out, Socket<Self::Request, Self::Response>)
    where
        Self: Sized,
        S: FnOnce(BoxFuture<'static, ()>) -> Out,
    {
        let (tx, rx) = mpsc::channel(64);
        let out = spawner(Box::pin(async move {
            run_unordered_async(rx, &self).await;
        }));
        let socket = Socket { sender: tx };
        (out, socket)
    }
}

/// A socket that can be used to communicate with a worker, you can use a socket to send
/// requests to a worker and wait for the response.
pub struct Socket<Req, Res> {
    sender: mpsc::Sender<Task<Req, Res>>,
}

/// Implementing [`Clone`] by hand because `#[derive(Clone)]` sucks for generics.
impl<Req, Res> Clone for Socket<Req, Res> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

/// A weak reference to a [`Socket`] that does not keep the worker alive.
pub struct WeakSocket<Req, Res> {
    sender: mpsc::WeakSender<Task<Req, Res>>,
}

/// Implementing [`Clone`] by hand because `#[derive(Clone)]` sucks for generics.
impl<Req, Res> Clone for WeakSocket<Req, Res> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RunError<Req> {
    /// The [`Socket`] failed to put the [`Req`] to be processed, this can be caused
    /// when the socket is closed.
    FailedToEnqueueReq(Req),
    /// The [`Socket`] failed to wait for the response, this can be due to the worker
    /// that is unexpectedly closed before processing a request.
    FailedToGetResponse,
}

impl<Req> Display for RunError<Req> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            RunError::FailedToEnqueueReq(_) => {
                write!(f, "failed to enqueue a request.")
            },
            RunError::FailedToGetResponse => {
                write!(f, "failed to recv the response")
            },
        }
    }
}

pub struct Task<Req, Res> {
    /// The raw request. Please consider using one of the `handle` functions instead.
    /// Access to this field will be deprecated.
    pub request: Req,
    respond: Option<oneshot::Sender<Res>>,
}

impl<Req, Res> Task<Req, Res> {
    /// Take any field related to the response and return a new task that has the unit type as a
    /// request parameter.
    #[inline(always)]
    fn split_to_responder(&mut self) -> Task<(), Res> {
        Task {
            request: (),
            respond: self.respond.take(),
        }
    }

    /// Respond to this task with the provided response.
    #[inline(always)]
    pub fn respond(self, response: Res) {
        if let Some(tx) = self.respond {
            let _ = tx.send(response);
        }
    }

    /// Handle the task from within the provided closure.
    #[inline(always)]
    pub fn handle<F>(mut self, handler: F)
    where
        F: FnOnce(Req) -> Res,
    {
        let responder = self.split_to_responder();
        let response = handler(self.request);
        responder.respond(response);
    }

    /// Handle the task from within the provided async closure.
    #[inline(always)]
    pub async fn handle_async<F, Fut>(mut self, handler: F)
    where
        F: FnOnce(Req) -> Fut,
        Fut: Future<Output = Res>,
    {
        let responder = self.split_to_responder();
        let response = handler(self.request).await;
        responder.respond(response);
    }
}

async fn run_non_blocking<Req, Res, H: Worker<Request = Req, Response = Res>>(
    mut rx: mpsc::Receiver<Task<Req, Res>>,
    mut handler: H,
) {
    while let Some(event) = rx.recv().await {
        tracing::info!("run_non_blocking before handle");
        event.handle(|req| handler.handle(req));
        tracing::info!("run_non_blocking after handle");
    }
}

async fn run_non_blocking_async<Req, Res, H: AsyncWorker<Request = Req, Response = Res>>(
    mut rx: mpsc::Receiver<Task<Req, Res>>,
    mut handler: H,
) {
    while let Some(event) = rx.recv().await {
        tracing::info!("run_non_blocking_async before handle");
        event.handle_async(|req| handler.handle(req)).await;
        tracing::info!("run_non_blocking_async after handle");
    }
}

async fn run_unordered_async<'a, H: AsyncWorkerUnordered + 'a>(
    mut rx: mpsc::Receiver<Task<H::Request, H::Response>>,
    handler: &'a H,
) {
    'outer: loop {
        let Some(event) = rx.recv().await else {
            return;
        };

        let mut futures = FuturesUnordered::<BoxFuture<'a, ()>>::new();
        let fut = event.handle_async(|req| handler.handle(req));
        futures.push(Box::pin(fut));

        'inner: loop {
            tokio::select! {
                maybe_event = rx.recv() => {
                    let Some(event) = maybe_event else {
                        break 'inner;
                    };
                    let fut = event.handle_async(|req| handler.handle(req));
                    futures.push(Box::pin(fut));
                },
                _ = futures.next() => {
                    tracing::info!("run_unordered_async next");
                    if futures.is_empty() {
                        continue 'outer;
                    }
                }
            }
        }

        if !futures.is_empty() {
            // drive all of the futures to completion.
            futures.count().await;
        }
    }
}

impl<Req, Res> Socket<Req, Res> {
    /// Returns a raw instance of [`Socket`] without a worker. The receiver is returned to
    /// the caller to handle the tasks.
    ///
    /// Once a task is received it is important to always use [`Task::respond`] in order to
    /// resolve the future returned to the caller in order to send the response back to the
    /// caller.
    pub fn raw_bounded(bound: usize) -> (Self, mpsc::Receiver<Task<Req, Res>>) {
        let (tx, rx) = mpsc::channel(bound);
        let socket = Socket { sender: tx };
        (socket, rx)
    }

    /// Downgrade this [`Socket`] into a [`WeakPort`] which does not keep the worker around if
    /// there are no [`Socket`]s anymore.
    pub fn downgrade(&self) -> WeakSocket<Req, Res> {
        WeakSocket {
            sender: self.sender.downgrade(),
        }
    }

    /// Enqueue a message to be processed by the worker without waiting for the response.
    pub async fn enqueue(&self, request: Req) -> Result<(), mpsc::error::SendError<Req>> {
        let event = Task {
            request,
            respond: None,
        };

        tracing::info!("### enqueue");
        self.sender
            .send(event)
            .await
            .map_err(|e| mpsc::error::SendError(e.0.request))
    }

    /// Enqueue a message to be processed by the worker and wait for the response.
    pub async fn run(&self, request: Req) -> Result<Res, RunError<Req>> {
        let (tx, rx) = oneshot::channel::<Res>();

        let event = Task {
            request,
            respond: Some(tx),
        };

        self.sender
            .send(event)
            .await
            .map_err(|e| RunError::FailedToEnqueueReq(e.0.request))?;

        let result = rx.await.map_err(|_| RunError::FailedToGetResponse)?;

        Ok(result)
    }
}

impl<Req, Res> WeakSocket<Req, Res> {
    /// Upgrade this weak socket into a [`Socket`], returns [`None`] if the socket is already
    /// dropped.
    pub fn upgrade(&self) -> Option<Socket<Req, Res>> {
        self.sender.upgrade().map(|sender| Socket { sender })
    }
}

impl<Req, Res> Unpin for Socket<Req, Res> {}
impl<Req, Res> Unpin for WeakSocket<Req, Res> {}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[derive(Default)]
    struct CounterWorker {
        current: u64,
    }

    impl Worker for CounterWorker {
        type Request = u64;
        type Response = u64;

        fn handle(&mut self, req: Self::Request) -> Self::Response {
            self.current += req;
            self.current
        }
    }

    impl AsyncWorker for CounterWorker {
        type Request = u64;
        type Response = u64;

        async fn handle(&mut self, req: Self::Request) -> Self::Response {
            self.current += req;
            self.current
        }
    }

    #[tokio::test]
    async fn test_async_unordered() {
        #[derive(Default)]
        struct Worker {}
        impl AsyncWorkerUnordered for Worker {
            type Request = u32;
            type Response = u32;

            async fn handle(&self, req: Self::Request) -> u32 {
                let (tx, rx) = tokio::sync::oneshot::channel();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    tx.send(req).unwrap();
                });
                rx.await.unwrap()
            }
        }

        let socket = Worker::default().spawn();
        let res = socket.run(0).await;
        assert_eq!(res.unwrap(), 0);
        let res = socket.run(1).await;
        assert_eq!(res.unwrap(), 1);
    }
}
