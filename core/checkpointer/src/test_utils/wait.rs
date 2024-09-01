use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::Result;

/// Polls until the given condition is met, or a timeout is reached.
///
/// # Errors
///
/// Returns an error if the timeout is reached.
pub fn wait_until<F, R>(condition: F, timeout: Duration, delay: Duration) -> Result<R>
where
    F: Fn() -> Option<R>,
{
    let start = Instant::now();

    while start.elapsed() < timeout {
        if let Some(result) = condition() {
            return Ok(result);
        }
        sleep(delay);
    }

    Err(anyhow::anyhow!("Timeout reached"))
}
