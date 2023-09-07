use std::process::Command;
use std::time::Duration;

use lightning_node::child_process::{ChildProcessConfig, ChildProcessRunner};
use lightning_node::shutdown::ShutdownController;

#[tokio::main]
async fn main() {
    if std::env::var("IS_CHILD").is_ok() {
        let shutdown = ShutdownController::default();
        shutdown.install_handlers();

        tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            println!("Oh noooooooooo");
            std::process::abort();
        });

        println!("CHILD {:?}", std::env::args());

        println!("child waiting for shutdown");
        shutdown.wait_for_shutdown().await;
        println!("child is down.");

        return;
    }

    let shutdown = ShutdownController::default();
    shutdown.install_handlers();

    let mut command = Command::new(std::env::current_exe().unwrap());
    command.env("IS_CHILD", "TRUE");
    command.args(std::env::args().skip(1));

    let config = ChildProcessConfig {
        name: "Lightning".to_owned(),
        command,
        shutdown_controller: Some(shutdown.clone()),
        callback: None,
        pid_file: None,
    };

    let mut child = ChildProcessRunner::new(config);
    child.start();

    shutdown.wait_for_shutdown().await;

    println!("Shutting node down.");
}
