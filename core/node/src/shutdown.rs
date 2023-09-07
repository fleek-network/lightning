//! The shutdown module makes it easy for us to have one source for sending shutdowns
//! signal to every sub service and have proper and graceful shutdowns in all of Lightning
//! services.

use std::sync::Arc;

use tokio::sync::Notify;

/// The shutdown controller can be used to be passed around in a program and provide
/// a unified signal for shutting down every single service and provide the application
/// with graceful shutdowns.
#[derive(Clone, Default)]
pub struct ShutdownController {
    pub notify: Arc<Notify>,
}

impl ShutdownController {
    /// Install the handler for control c to submit trigger this shutdown handler.
    pub fn install_handlers(&self) {
        tracing::debug!("Registering shutdown signals");

        let notify = self.notify.clone();
        tokio::task::spawn(async move {
            shutdown_stream().await;
            notify.notify_waiters();
        });
    }

    /// Manually send the shutdown signal.
    pub fn shutdown(&self) {
        self.notify.notify_waiters();
    }

    /// Wait for the shutdown signal to be sent.
    pub async fn wait_for_shutdown(self) {
        tracing::info!("Waiting for shutdown signal");
        let future = self.notify.notified();
        tokio::pin!(future);
        future.as_mut().await;
    }
}

#[cfg(unix)]
/// The shutdown controller for Unix that listens for:
/// - SIGINT (Ctrl + C)
/// - SIGQUIT (Ctrl + D)
/// - SIGTERM (sent by `kill` by default)
async fn shutdown_stream() {
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
async fn shutdown_stream() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to setup control-c handler.");
    tracing::info!("Received ctrl-c signal.");
}
