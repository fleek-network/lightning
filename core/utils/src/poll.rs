use std::future::Future;

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PollUntilError {
    #[error("Condition error: {0:?}")]
    ConditionError(String),

    #[error("Condition not satisfied")]
    ConditionNotSatisfied,

    #[error("Timeout reached")]
    Timeout,
}

/// Polls until the given condition is satisfied, or a timeout is reached.
///
/// The condition is expected to return a `Result<R, PollUntilError>`. If the condition returns an
/// error, the error is propagated and the condition is not re-evaluated. If the condition returns
/// `PollUntilError::ConditionNotSatisfied`, the condition is re-evaluated after the delay.
///
/// Returns `PollUntilError::Timeout` if the timeout is reached.
pub async fn poll_until<F, Fut, R>(
    condition: F,
    timeout: tokio::time::Duration,
    delay: tokio::time::Duration,
) -> Result<R, PollUntilError>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<R, PollUntilError>>,
{
    let start = tokio::time::Instant::now();

    while start.elapsed() < timeout {
        match condition().await {
            Ok(result) => return Ok(result),
            Err(PollUntilError::ConditionNotSatisfied) => {
                tokio::time::sleep(delay).await;
                continue;
            },
            Err(e) => return Err(e),
        }
    }

    Err(PollUntilError::Timeout)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn test_condition_returns_success_immediately() {
        let condition = || async { Ok(42) };
        let timeout = Duration::from_secs(1);
        let delay = Duration::from_millis(100);

        let result = poll_until(condition, timeout, delay).await;
        assert_eq!(result, Ok(42));
    }

    #[tokio::test]
    async fn test_condition_returns_success_eventually_before_timeout() {
        let count = AtomicUsize::new(0);
        let condition = || async {
            count.fetch_add(1, Ordering::Relaxed);
            (count.load(Ordering::Relaxed) >= 3)
                .then_some(())
                .ok_or(PollUntilError::ConditionNotSatisfied)
        };

        let result: Result<(), PollUntilError> = poll_until(
            condition,
            Duration::from_secs(1),
            Duration::from_millis(100),
        )
        .await;
        assert!(result.is_ok());
        assert_eq!(count.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn test_condition_returns_error() {
        let condition = || async { Err(PollUntilError::ConditionError("test".to_string())) };
        let timeout = Duration::from_secs(1);
        let delay = Duration::from_millis(100);

        let result: Result<(), PollUntilError> = poll_until(condition, timeout, delay).await;
        assert_eq!(
            result,
            Err(PollUntilError::ConditionError("test".to_string()))
        );
    }

    #[tokio::test]
    async fn test_condition_returns_not_satisfied_until_timeout() {
        let count = AtomicUsize::new(0);
        let condition = || async {
            count.fetch_add(1, Ordering::Relaxed);
            Err(PollUntilError::ConditionNotSatisfied)
        };

        let result: Result<(), PollUntilError> = poll_until(
            condition,
            Duration::from_secs(1),
            Duration::from_millis(100),
        )
        .await;
        assert_eq!(result, Err(PollUntilError::Timeout));
        assert_eq!(count.load(Ordering::Relaxed), 10);
    }
}
