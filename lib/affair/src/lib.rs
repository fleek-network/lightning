//! A simple tokio-based library to spawn a worker and communicate with it, the worker
//! can be spawned on a dedicated thread or as a tokio task.
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
//!     let port = DedicatedThread::spawn(CounterWorker::default());
//!     assert_eq!(port.run(10).await.unwrap(), 10);
//!     assert_eq!(port.run(3).await.unwrap(), 13);
//! }
//! ```

use std::fmt::Display;

use async_trait::async_trait;
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
}

/// An awaiting worker that processes requests and returns a response.
#[async_trait]
pub trait AsyncWorker: Send + 'static {
    /// The payload of the request that can be sent to the worker.
    type Request: Send + 'static;

    /// The response for a request.
    type Response: Send + 'static;

    /// Implement your custom handler for this async worker and enjoy using `await`.
    async fn handle(&mut self, req: Self::Request) -> Self::Response;
}

/// A port that can be used to communicate with a worker, you can use a port to send
/// requests to a worker and wait for the response.
#[derive(Clone)]
pub struct Port<Req, Res> {
    sender: mpsc::Sender<Task<Req, Res>>,
}

/// A weak reference to a [`Port`] that does not keep the worker alive.
#[derive(Clone)]
pub struct WeakPort<Req, Res> {
    sender: mpsc::WeakSender<Task<Req, Res>>,
}

#[derive(Debug, Clone, Copy)]
pub enum RunError<Req> {
    /// The [`Port`] failed to put the [`Req`] to be processed, this can be caused
    /// when the port is closed.
    FailedToEnqueueReq(Req),
    /// The [`Port`] failed to wait for the response, this can be due to the worker
    /// that is unexpectedly closed before processing a request.
    FailedToGetResponse,
}

impl<Req> Display for RunError<Req> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            RunError::FailedToEnqueueReq(_) => {
                write!(f, "failed to enqueue a request.")
            }
            RunError::FailedToGetResponse => {
                write!(f, "failed to recv the response")
            }
        }
    }
}

/// An execution engine that handles spawning a [`Worker`].
pub trait Executor {
    /// Spawn a [`Worker`] and returns a [`Port`] which can be used to send requests to the spawned
    /// worker.
    ///
    /// The worker is stopped when all of the references to [`Port`] are dropped, if you want a
    /// port that does not keep the worker alive consider using a [`WeakPort`].
    fn spawn<H: Worker>(handler: H) -> Port<H::Request, H::Response>;

    /// Spawn a [`AsyncWorker`] and returns a [`Port`] which can be used to send requests to the
    /// spawned worker.
    ///
    /// The worker is stopped when all of the references to [`Port`] are dropped, if you want a
    /// port that does not keep the worker alive consider using a [`WeakPort`].
    fn spawn_async<H: AsyncWorker>(handler: H) -> Port<H::Request, H::Response>;
}

struct Task<Req, Res> {
    request: Req,
    respond: Option<oneshot::Sender<Res>>,
}

/// An executor that provides the worker with its dedicated std::thread.
#[derive(Clone, Copy, Debug)]
pub struct DedicatedThread;

/// An executor that runs the worker as a tokio task.
#[derive(Clone, Copy, Debug)]
pub struct TokioSpawn;

impl Executor for DedicatedThread {
    fn spawn<H: Worker>(handler: H) -> Port<H::Request, H::Response> {
        let (tx, rx) = mpsc::channel(64);
        std::thread::spawn(|| run_blocking(rx, handler));
        Port { sender: tx }
    }

    fn spawn_async<H: AsyncWorker>(handler: H) -> Port<H::Request, H::Response> {
        let (tx, rx) = mpsc::channel(64);
        std::thread::spawn(|| run_blocking_async(rx, handler));
        Port { sender: tx }
    }
}

impl Executor for TokioSpawn {
    fn spawn<H: Worker>(handler: H) -> Port<H::Request, H::Response> {
        let (tx, rx) = mpsc::channel(64);
        tokio::spawn(run_non_blocking(rx, handler));
        Port { sender: tx }
    }

    fn spawn_async<H: AsyncWorker>(handler: H) -> Port<H::Request, H::Response> {
        let (tx, rx) = mpsc::channel(64);
        tokio::spawn(run_non_blocking_async(rx, handler));
        Port { sender: tx }
    }
}

fn run_blocking<Req, Res, H: Worker<Request = Req, Response = Res>>(
    mut rx: mpsc::Receiver<Task<Req, Res>>,
    mut handler: H,
) {
    while let Some(event) = rx.blocking_recv() {
        let res = handler.handle(event.request);
        if let Some(tx) = event.respond {
            let _ = tx.send(res);
        }
    }
}

fn run_blocking_async<Req, Res, H: AsyncWorker<Request = Req, Response = Res>>(
    mut rx: mpsc::Receiver<Task<Req, Res>>,
    mut handler: H,
) {
    while let Some(event) = rx.blocking_recv() {
        let res = futures::executor::block_on(handler.handle(event.request));
        if let Some(tx) = event.respond {
            let _ = tx.send(res);
        }
    }
}

async fn run_non_blocking<Req, Res, H: Worker<Request = Req, Response = Res>>(
    mut rx: mpsc::Receiver<Task<Req, Res>>,
    mut handler: H,
) {
    while let Some(event) = rx.recv().await {
        let result = handler.handle(event.request);
        if let Some(tx) = event.respond {
            let _ = tx.send(result);
        }
    }
}

async fn run_non_blocking_async<Req, Res, H: AsyncWorker<Request = Req, Response = Res>>(
    mut rx: mpsc::Receiver<Task<Req, Res>>,
    mut handler: H,
) {
    while let Some(event) = rx.recv().await {
        let result = handler.handle(event.request).await;
        if let Some(tx) = event.respond {
            let _ = tx.send(result);
        }
    }
}

impl<Req, Res> Port<Req, Res> {
    /// Downgrade this [`Port`] into a [`WeakPort`] which does not keep the worker around if
    /// there are no [`Port`]s anymore.
    pub fn downgrade(&self) -> WeakPort<Req, Res> {
        WeakPort {
            sender: self.sender.downgrade(),
        }
    }

    /// Enqueue a message to be processed by the worker without waiting for the response.
    pub async fn enqueue(&self, request: Req) -> Result<(), mpsc::error::SendError<Req>> {
        let event = Task {
            request,
            respond: None,
        };

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

impl<Req, Res> WeakPort<Req, Res> {
    /// Upgrade this weak port into a [`Port`], returns [`None`] if the port is already dropped.
    pub fn upgrade(&self) -> Option<Port<Req, Res>> {
        self.sender.upgrade().map(|sender| Port { sender })
    }
}

impl<Req, Res> Unpin for Port<Req, Res> {}
impl<Req, Res> Unpin for WeakPort<Req, Res> {}

#[cfg(test)]
mod tests {
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

    #[async_trait]
    impl AsyncWorker for CounterWorker {
        type Request = u64;
        type Response = u64;

        async fn handle(&mut self, req: Self::Request) -> Self::Response {
            self.current += req;
            self.current
        }
    }

    #[tokio::test]
    async fn test_dedicated_thread() {
        let port = DedicatedThread::spawn(CounterWorker::default());
        assert_eq!(port.run(10).await.unwrap(), 10);
        assert_eq!(port.run(3).await.unwrap(), 13);

        let port = DedicatedThread::spawn_async(CounterWorker::default());
        assert_eq!(port.run(10).await.unwrap(), 10);
        assert_eq!(port.run(3).await.unwrap(), 13);
    }

    #[tokio::test]
    async fn test_tokio_spawn() {
        let port = TokioSpawn::spawn(CounterWorker::default());
        assert_eq!(port.run(10).await.unwrap(), 10);
        assert_eq!(port.run(3).await.unwrap(), 13);

        let port = DedicatedThread::spawn_async(CounterWorker::default());
        assert_eq!(port.run(10).await.unwrap(), 10);
        assert_eq!(port.run(3).await.unwrap(), 13);
    }
}
