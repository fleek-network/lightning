//! This module provides an implementation of [`shutdown_stream`] under different platforms (unix
//! and windows).
//!
//! Currently, this relies on tokio.

#[cfg(unix)]
/// The shutdown controller for Unix that listens for:
/// - SIGINT (Ctrl + C)
/// - SIGQUIT (Ctrl + D)
/// - SIGTERM (sent by `kill` by default)
pub async fn shutdown_stream() {
    use tokio::signal::unix::{signal, SignalKind};
    // ctrl+c
    let mut interrupt_signal =
        signal(SignalKind::interrupt()).expect("Failed to setup INTERRUPT handler.");

    let mut terminate_signal =
        signal(SignalKind::terminate()).expect("Failed to setup TERMINATE handler.");

    let mut quit_signal = signal(SignalKind::quit()).expect("Failed to setup QUIT handler.");

    tokio::select! {
        _ = interrupt_signal.recv() => {
            tracing::info!("Received ctrl-c signal.");
        }
        _ = terminate_signal.recv() => {
            tracing::info!("Received SIGTERM signal.");
        }
        _ = quit_signal.recv() => {
            tracing::info!("Received SIGQUIT signal.");
        }
    }
}

#[cfg(windows)]
/// On windows only listen for ctrl-c for now.
pub async fn shutdown_stream() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to setup control-c handler.");
    tracing::info!("Received ctrl-c signal.");
}
