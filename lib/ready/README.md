# Ready

A simple library for waiting for a state to change.

This is a single-use notification. After the first notification, the waiter is considered ready and any further notifications are ignored.

## Usage

```rust
use ready::tokio::TokioReadyWaiter;
use ready::ReadyWaiter;

#[derive(Clone, Debug, Default)]
struct TestReadyState {
    hello: String,
}

#[tokio::main]
async fn main() {
    // Create a ready signal.
    let ready = TokioReadyWaiter::new();

    {
        // Clone the ready signal so that the task can notify the original using the same reference.
        let ready = ready.clone();

        // Spawn a task that will notify the original ready signal after a delay.
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let state = TestReadyState {
                hello: "world".to_string(),
            };
            ready.notify(state);
        });
    }

    // Wait for the ready signal to be notified.
    let state = ready.wait().await;
    println!("hello, {}!", state.hello);
}
```
