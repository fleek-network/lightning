use std::future::Future;

use anyhow::Result;
use thiserror::Error;

/// Polls until the given condition is met, or a timeout is reached.
///
/// This is a blocking, synchronous operation.
///
/// # Errors
///
/// Returns an error if the timeout is reached.
#[allow(unused)]
pub fn sync_wait_until<F, R>(
    condition: F,
    timeout: std::time::Duration,
    delay: std::time::Duration,
) -> Result<R, WaitUntilError>
where
    F: Fn() -> Option<R>,
{
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if let Some(result) = condition() {
            return Ok(result);
        }
        std::thread::sleep(delay);
    }

    Err(WaitUntilError::Timeout)
}

/// Polls asynchronously until the given condition is met, or a timeout is reached.
///
/// This is a non-blocking, asynchronous operation.
///
/// # Errors
///
/// Returns an error if the timeout is reached.
pub async fn wait_until<F, Fut, R>(
    condition: F,
    timeout: tokio::time::Duration,
    delay: tokio::time::Duration,
) -> Result<R, WaitUntilError>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Option<R>>,
{
    let start = tokio::time::Instant::now();

    while start.elapsed() < timeout {
        if let Some(result) = condition().await {
            return Ok(result);
        }
        tokio::time::sleep(delay).await;
    }

    Err(WaitUntilError::Timeout)
}

/// The error type returned by `wait_until` methods.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum WaitUntilError {
    #[error("Timeout reached")]
    Timeout,
}
