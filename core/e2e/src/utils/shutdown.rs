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
            println!("Received ctrl-c signal.");
        }
        _ = terminate_signal.recv() => {
            println!("Received SIGTERM signal.");
        }
        _ = quit_signal.recv() => {
            println!("Received SIGQUIT signal.");
        }
    }
}
