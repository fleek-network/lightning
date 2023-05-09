# Affair

A simple tokio-based library to spawn a worker and communicate with it, the worker
can be spawned on a dedicated thread or as a tokio task.

# Example

```rust
use affair::*;

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

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let port = DedicatedThread::spawn(CounterWorker::default());
    assert_eq!(port.run(10).await.unwrap(), 10);
    assert_eq!(port.run(3).await.unwrap(), 13);
}
```
