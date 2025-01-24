use std::fs;
use std::path::Path;
use std::process::Command;

use lightning_utils::poll::{poll_until, PollUntilError};
use minilsof::LsofData;
use thiserror::Error;
use tokio::time::Duration;

/// Wait until the given file is no longer open by any process.
///
/// This function repeatedly checks whether the file at the given path has any open file descriptors
/// associated with it, waiting until the file is fully closed or the timeout is reached.
///
/// It does this using `/proc` directly via the `minilsof` crate if available, otherwise it shells
/// out to `lsof`. There is unforunately no pure-rust way to do this across platforms.
pub async fn wait_for_file_to_close(
    path: &Path,
    timeout: Duration,
    delay: Duration,
) -> Result<(), WaitForFileToCloseError> {
    if is_proc_available() {
        wait_for_file_to_close_with_proc(path, timeout, delay).await
    } else {
        wait_for_file_to_close_with_lsof(path, timeout, delay).await
    }
}

/// Check if `/proc` is available on the current system.
fn is_proc_available() -> bool {
    fs::metadata("/proc").is_ok_and(|meta| meta.is_dir())
}

/// Wait until the given file is no longer open by any process, using the `/proc` filesystem to get
/// the processes that have the file open.
///
/// This function repeatedly checks whether the file at the given path has any open file descriptors
/// associated with it, waiting until the file is fully closed or the timeout is reached.
async fn wait_for_file_to_close_with_proc(
    path: &Path,
    timeout: Duration,
    delay: Duration,
) -> Result<(), WaitForFileToCloseError> {
    poll_until(
        || async {
            if !path.exists() {
                return Ok(());
            }

            let path = path.to_str().ok_or(WaitForFileToCloseError::Internal(
                "failed to convert path to string".to_string(),
            ))?;
            let fds = LsofData::new()
                .target_file_ls(path.to_string())
                .unwrap_or_default();

            if fds.is_empty() {
                Ok(())
            } else {
                tracing::debug!("file is still open: {:?}", path);
                Err(PollUntilError::ConditionNotSatisfied)
            }
        },
        timeout,
        delay,
    )
    .await?;

    Ok(())
}

/// Wait until the given file is no longer open by any process, using the `lsof` command to get the
/// processes that have the file open.
///
/// This function repeatedly checks whether the file at the given path has any open file descriptors
/// associated with it, waiting until the file is fully closed or the timeout is reached.
///
/// This is a fallback for when `/proc` is not available, since it's not ideal to shell out to
/// `lsof` to accomplish this.
async fn wait_for_file_to_close_with_lsof(
    path: &Path,
    timeout: Duration,
    delay: Duration,
) -> Result<(), WaitForFileToCloseError> {
    poll_until(
        || async {
            if !path.exists() {
                return Ok(());
            }

            let output = Command::new("lsof")
                .arg(path.to_str().unwrap())
                .output()
                .map_err(|e| WaitForFileToCloseError::Internal(e.to_string()))?;

            if output.status.code() == Some(1) {
                return Ok(());
            }

            if output.status.code() == Some(0) {
                tracing::debug!("file is still open: {:?}", path);
                return Err(PollUntilError::ConditionNotSatisfied);
            }

            Err(WaitForFileToCloseError::Internal(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))?
        },
        timeout,
        delay,
    )
    .await?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum WaitForFileToCloseError {
    #[error("Timeout reached")]
    Timeout,

    #[error("Internal: {0:?}")]
    Internal(String),
}

impl From<PollUntilError> for WaitForFileToCloseError {
    fn from(error: PollUntilError) -> Self {
        match error {
            PollUntilError::Timeout => Self::Timeout,
            PollUntilError::ConditionError(e) => Self::Internal(e),
            PollUntilError::ConditionNotSatisfied => {
                unreachable!()
            },
        }
    }
}

impl From<WaitForFileToCloseError> for PollUntilError {
    fn from(error: WaitForFileToCloseError) -> Self {
        match error {
            WaitForFileToCloseError::Timeout => Self::Timeout,
            WaitForFileToCloseError::Internal(e) => Self::ConditionError(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use tempfile::tempdir;

    use super::*;

    #[tokio::test]
    async fn test_wait_for_file_to_close() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join("test.txt");
        let file = File::create(&path).unwrap();

        // Check that the file is open.
        let result =
            wait_for_file_to_close(&path, Duration::from_millis(200), Duration::from_millis(50))
                .await;
        assert!(matches!(
            result.unwrap_err(),
            WaitForFileToCloseError::Timeout
        ));

        // Drop and close the file.
        drop(file);

        // Check that it's now open.
        wait_for_file_to_close(&path, Duration::from_secs(1), Duration::from_millis(50))
            .await
            .unwrap();
    }
}
