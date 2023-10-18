use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use fn_sdk::ipc_types::{self, IpcMessage};
use tokio::io::Interest;
use tokio::net::{UnixListener, UnixStream};
use tokio::process::Command;
use tokio::sync::{mpsc, Notify};
use tokio::task::JoinHandle;
use tokio::{pin, select};
use triomphe::Arc;

type WorkerSocket = affair::Socket<ipc_types::Request, ipc_types::Response>;

/// The shared object with every service.
pub struct Context {
    kill: Arc<Notify>,
    worker: WorkerSocket,
    blockstore_path: PathBuf,
    ipc_path: PathBuf,
}

/// Collection of every service that we have.
#[derive(Clone, Default)]
pub struct ServiceCollection {
    services: Arc<DashMap<u32, Arc<ServiceHandle>, fxhash::FxBuildHasher>>,
}

pub struct ServiceHandle {
    sender: mpsc::Sender<ipc_types::IpcMessage>,
}

pub async fn spawn_service(id: u32, cx: Arc<Context>) -> ServiceHandle {
    let ipc_dir = cx.ipc_path.join(format!("service-{id}"));
    tracing::trace!("Spawning service {id} [IPC={ipc_dir:?}.");

    // First try to remove the directory if it does exits. And then create it
    // again. Ignoring the errors that may occur while removing.
    let _ = tokio::fs::remove_dir_all(&ipc_dir).await;
    tokio::fs::create_dir_all(&ipc_dir)
        .await
        .expect("Failed to create IPC directory for service.");

    let mut args = std::env::args_os();
    let program = args.next().unwrap();
    let command = {
        let mut cmd = Command::new(program);
        cmd.args(args)
            .env("SERVICE_ID", format!("{id}"))
            .env("BLOCKSTORE_PATH", &cx.blockstore_path)
            .env("IPC_PATH", &ipc_dir);
        cmd
    };

    let kill_notify = cx.kill.clone();
    let cmd_permit = Arc::new(Notify::new());
    let permit = cmd_permit.clone();
    tokio::spawn(async move {
        // Wait until we have the UDS listener listening.
        permit.notified().await;
        tracing::trace!("Got UDS signal for '{id}'. Starting the service process");
        run_command(format!("service-{id}"), command, kill_notify).await;
        tracing::trace!("Task {id} was killed.");
    });

    let (tx, rx) = mpsc::channel::<IpcMessage>(128);

    tokio::spawn(async move {
        run_ctrl_loop(&ipc_dir, cx, cmd_permit, rx).await;
    });

    todo!()
}

async fn run_ctrl_loop(
    ipc_path: &Path,
    cx: Arc<Context>,
    cmd_permit: Arc<Notify>,
    rx: mpsc::Receiver<ipc_types::IpcMessage>,
) {
    let ctrl_path = ipc_path.join("ctrl");
    // The file might not exist.
    let _ = tokio::fs::remove_file(ctrl_path).await;

    let listener = UnixListener::bind(ipc_path).expect("Failed to bind to IPC socket.");
    cmd_permit.notify_one();

    pin! {
        let kill_fut = cx.kill.notified();
    };

    enum Status {
        NotRunning(mpsc::Receiver<ipc_types::IpcMessage>),
        Running(JoinHandle<mpsc::Receiver<ipc_types::IpcMessage>>),
    }

    let killer = Arc::new(Notify::new());

    // Every time the service process is restarted (due to failures). The next one will attempt to
    // create a new connection to the same socket and here we will catch it.
    loop {
        tokio::select! {
            _ = &mut kill_fut => {
                break;
            },
            Ok((stream, _)) = listener.accept() => {
                tokio::spawn(handle_stream(stream, cx.worker.clone(), ));
            }
        }
    }

    // We got the kill signal. So let's do some cleanup.
    drop(listener);
}

async fn handle_stream(
    stream: UnixStream,
    _worker: WorkerSocket,
    mut rx: mpsc::Receiver<ipc_types::IpcMessage>,
    kill: Arc<Notify>,
) -> mpsc::Receiver<ipc_types::IpcMessage> {
    let _ = stream.ready(Interest::WRITABLE).await;

    rx
}

/// Run the given command until the kill signal has been received. Restarting the child on failure.
async fn run_command(name: String, mut command: Command, kill: Arc<Notify>) {
    pin! {
        let kill_fut = kill.notified();
    };

    command
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let mut wait_time = 1000;
    loop {
        let last_start = Instant::now();
        tracing::trace!("Running command for '{name}");

        let mut child = command.spawn().expect("Command failed to start.");

        select! {
            _ = &mut kill_fut => {
                tracing::trace!("Got the signal to kill. Killing '{name}'");
                child.kill().await.expect("Failed to kill the child.");
                tracing::trace!("Killed process '{name}'");
                break;
            },
            _ = child.wait() => {
                tracing::error!("Child process '{name}' failed.");
                if Instant::now().duration_since(last_start) >= Duration::from_secs(2) {
                    tracing::info!("Last run seemed to have been healthy, not waiting...");
                    // Restart the wait time.
                    wait_time = 1000;
                    continue;
                }
            }
        }

        let wait_dur = Duration::from_millis(wait_time);
        tracing::info!("Waiting for {wait_dur:?} before restarting '{name}'");

        select! {
            _ = &mut kill_fut => {
                tracing::trace!("Got the signal to stop '{name}'");
                break;
            }
            _ = tokio::time::sleep(wait_dur) => {
                wait_time = wait_time * 12 / 10;
            }
        }
    }
}
