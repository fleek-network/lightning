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
//!     let socket = DedicatedThread::spawn(CounterWorker::default());
//!     assert_eq!(socket.run(10).await.unwrap(), 10);
//!     assert_eq!(socket.run(3).await.unwrap(), 13);
//! }
//! ```

use std::fmt::Display;
use std::sync::{Arc, Weak};

use async_trait::async_trait;
use crossbeam::sync::ShardedLock;
use futures::Future;
use tokio::sync::{mpsc, oneshot};

pub type Observer<Res> = fn(&Res);
type Observers<Res> = ShardedLock<Vec<Observer<Res>>>;

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

/// A socket that can be used to communicate with a worker, you can use a socket to send
/// requests to a worker and wait for the response.
pub struct Socket<Req, Res> {
    sender: mpsc::Sender<Task<Req, Res>>,
    observers: Arc<Observers<Res>>,
}

/// Implementing [`Clone`] by hand because `#[derive(Clone)]` sucks for generics.
impl<Req, Res> Clone for Socket<Req, Res> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            observers: self.observers.clone(),
        }
    }
}

/// A weak reference to a [`Socket`] that does not keep the worker alive.
pub struct WeakSocket<Req, Res> {
    sender: mpsc::WeakSender<Task<Req, Res>>,
    observers: Weak<Observers<Res>>,
}

/// Implementing [`Clone`] by hand because `#[derive(Clone)]` sucks for generics.
impl<Req, Res> Clone for WeakSocket<Req, Res> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            observers: self.observers.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
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

/// An execution engine that handles spawning a [`Worker`].
pub trait Executor {
    /// Spawn a [`Worker`] and returns a [`Socket`] which can be used to send requests to the
    /// spawned worker.
    ///
    /// The worker is stopped when all of the references to [`Socket`] are dropped, if you want a
    /// socket that does not keep the worker alive consider using a [`WeakSocket`].
    fn spawn<H: Worker>(handler: H) -> Socket<H::Request, H::Response>;

    /// Spawn a [`AsyncWorker`] and returns a [`Socket`] which can be used to send requests to the
    /// spawned worker.
    ///
    /// The worker is stopped when all of the references to [`Socket`] are dropped, if you want a
    /// socket that does not keep the worker alive consider using a [`WeakSocket`].
    fn spawn_async<H: AsyncWorker>(handler: H) -> Socket<H::Request, H::Response>;
}

pub struct Task<Req, Res> {
    /// The raw request. Please consider using one of the `handle` functions instead.
    /// Access to this field will be deprecated.
    pub request: Req,
    respond: Option<oneshot::Sender<Res>>,
    observers: Option<Arc<Observers<Res>>>,
}

impl<Req, Res> Task<Req, Res> {
    /// Take any field related to the response and return a new task that has the unit type as a
    /// request parameter.
    #[inline(always)]
    fn split_to_responder(&mut self) -> Task<(), Res> {
        Task {
            request: (),
            respond: self.respond.take(),
            observers: self.observers.take()
        }
    }

    /// Respond to this task with the provided response.
    #[inline(always)]
    pub fn respond(self, response: Res) {
        if let Some(observers) = self.observers {
            let guard = observers.read().expect("Unexpected poisioned lock.");
            for cb in guard.iter() {
                (*cb)(&response);
            }
        }

        if let Some(tx) = self.respond {
            let _ = tx.send(response);
        }
    }

    /// Handle the task from within the provided closure.
    #[inline(always)]
    pub fn handle<F>(mut self, handler: F) where F: FnOnce(Req) -> Res {
        let responder = self.split_to_responder();
        let response = handler(self.request);
        responder.respond(response);
    }

    /// Handle the task from within the provided async closure.
    #[inline(always)]
    pub async fn handle_async<F, Fut>(mut self, handler: F) where F: FnOnce(Req) -> Fut, Fut: Future<Output = Res> {
        let responder = self.split_to_responder();
        let response = handler(self.request).await;
        responder.respond(response);
    }
}

/// An executor that provides the worker with its dedicated std::thread.
#[derive(Clone, Copy, Debug)]
pub struct DedicatedThread;

/// An executor that runs the worker as a tokio task.
#[derive(Clone, Copy, Debug)]
pub struct TokioSpawn;

impl Executor for DedicatedThread {
    fn spawn<H: Worker>(handler: H) -> Socket<H::Request, H::Response> {
        let (tx, rx) = mpsc::channel(64);
        std::thread::spawn(|| run_blocking(rx, handler));
        Socket {
            sender: tx,
            observers: Default::default(),
        }
    }

    fn spawn_async<H: AsyncWorker>(handler: H) -> Socket<H::Request, H::Response> {
        let (tx, rx) = mpsc::channel(64);
        std::thread::spawn(|| run_blocking_async(rx, handler));
        Socket {
            sender: tx,
            observers: Default::default(),
        }
    }
}

impl Executor for TokioSpawn {
    fn spawn<H: Worker>(handler: H) -> Socket<H::Request, H::Response> {
        let (tx, rx) = mpsc::channel(64);
        tokio::spawn(run_non_blocking(rx, handler));
        Socket {
            sender: tx,
            observers: Default::default(),
        }
    }

    fn spawn_async<H: AsyncWorker>(handler: H) -> Socket<H::Request, H::Response> {
        let (tx, rx) = mpsc::channel(64);
        tokio::spawn(run_non_blocking_async(rx, handler));
        Socket {
            sender: tx,
            observers: Default::default(),
        }
    }
}

fn run_blocking<Req, Res, H: Worker<Request = Req, Response = Res>>(
    mut rx: mpsc::Receiver<Task<Req, Res>>,
    mut handler: H,
) {
    while let Some(event) = rx.blocking_recv() {
        event.handle(|req| handler.handle(req));
    }
}

fn run_blocking_async<Req, Res, H: AsyncWorker<Request = Req, Response = Res>>(
    mut rx: mpsc::Receiver<Task<Req, Res>>,
    mut handler: H,
) {
    while let Some(event) = rx.blocking_recv() {
        event.handle(|req| futures::executor::block_on(handler.handle(req)));
    }
}

async fn run_non_blocking<Req, Res, H: Worker<Request = Req, Response = Res>>(
    mut rx: mpsc::Receiver<Task<Req, Res>>,
    mut handler: H,
) {
    while let Some(event) = rx.recv().await {
        event.handle(|req| handler.handle(req));
    }
}

async fn run_non_blocking_async<Req, Res, H: AsyncWorker<Request = Req, Response = Res>>(
    mut rx: mpsc::Receiver<Task<Req, Res>>,
    mut handler: H,
) {
    while let Some(event) = rx.recv().await {
        event.handle_async(|req| handler.handle(req)).await;
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
        let socket = Socket {
            sender: tx,
            observers: Default::default(),
        };
        (socket, rx)
    }

    /// Downgrade this [`Socket`] into a [`WeakPort`] which does not keep the worker around if
    /// there are no [`Socket`]s anymore.
    pub fn downgrade(&self) -> WeakSocket<Req, Res> {
        WeakSocket {
            sender: self.sender.downgrade(),
            observers: Arc::downgrade(&self.observers),
        }
    }

    /// Enqueue a message to be processed by the worker without waiting for the response.
    pub async fn enqueue(&self, request: Req) -> Result<(), mpsc::error::SendError<Req>> {
        let event = Task {
            request,
            respond: None,
            // Since we do not want to wait for the response delegate the making calls to
            // observers to the worker thread.
            observers: Some(self.observers.clone()),
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
            observers: None,
        };

        self.sender
            .send(event)
            .await
            .map_err(|e| RunError::FailedToEnqueueReq(e.0.request))?;

        let result = rx.await.map_err(|_| RunError::FailedToGetResponse)?;

        // TODO(qti3e): Optimize this hot path.
        let guard = self.observers.read().expect("Unexpected poisioned lock.");
        for cb in guard.iter() {
            (*cb)(&result);
        }

        Ok(result)
    }

    /// Inject an observer that can view the responses and on them.
    pub fn inject(&self, observer: Observer<Res>) {
        let mut guard = self.observers.write().expect("Unexpected posioned lock.");
        guard.push(observer);
    }
}

impl<Req, Res> WeakSocket<Req, Res> {
    /// Upgrade this weak socket into a [`Socket`], returns [`None`] if the socket is already
    /// dropped.
    pub fn upgrade(&self) -> Option<Socket<Req, Res>> {
        self.sender
            .upgrade()
            .zip(self.observers.upgrade())
            .map(|(sender, observers)| Socket { sender, observers })
    }
}

impl<Req, Res> Unpin for Socket<Req, Res> {}
impl<Req, Res> Unpin for WeakSocket<Req, Res> {}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

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
        let socket = DedicatedThread::spawn(CounterWorker::default());
        assert_eq!(socket.run(10).await.unwrap(), 10);
        assert_eq!(socket.run(3).await.unwrap(), 13);

        let socket = DedicatedThread::spawn_async(CounterWorker::default());
        assert_eq!(socket.run(10).await.unwrap(), 10);
        assert_eq!(socket.run(3).await.unwrap(), 13);
    }

    #[tokio::test]
    async fn test_tokio_spawn() {
        let socket = TokioSpawn::spawn(CounterWorker::default());
        assert_eq!(socket.run(10).await.unwrap(), 10);
        assert_eq!(socket.run(3).await.unwrap(), 13);

        let socket = DedicatedThread::spawn_async(CounterWorker::default());
        assert_eq!(socket.run(10).await.unwrap(), 10);
        assert_eq!(socket.run(3).await.unwrap(), 13);
    }

    #[tokio::test]
    async fn inject_should_work() {
        static TAPE: Mutex<Vec<u64>> = Mutex::new(Vec::new());

        fn observer(value: &u64) {
            TAPE.lock().unwrap().push(*value);
        }

        let socket = TokioSpawn::spawn(CounterWorker::default());
        let socket2 = socket.downgrade().upgrade().unwrap();
        socket2.inject(observer);

        assert_eq!(socket.run(10).await.unwrap(), 10);
        assert_eq!(TAPE.lock().unwrap().as_ref(), vec![10]);

        assert_eq!(socket.run(3).await.unwrap(), 13);
        assert_eq!(TAPE.lock().unwrap().as_ref(), vec![10, 13]);

        socket.enqueue(5).await.unwrap();

        assert_eq!(socket.run(2).await.unwrap(), 20);
        assert_eq!(TAPE.lock().unwrap().as_ref(), vec![10, 13, 18, 20]);
    }
}
