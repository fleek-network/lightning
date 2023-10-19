use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use fn_sdk::ipc_types::{self, IpcMessage};
use futures::Future;
use tokio::io::{self, Interest};
use tokio::net::{UnixListener, UnixStream};
use tokio::process::Command;
use tokio::sync::Notify;
use tokio::task::JoinSet;
use tokio::{pin, select};
use triomphe::Arc;

/// The shared object with every service.
pub struct Context {
    pub kill: Arc<Notify>,
    pub blockstore_path: PathBuf,
    pub ipc_path: PathBuf,
}

/// Collection of every service that we have.
#[derive(Clone, Default)]
pub struct ServiceCollection {
    services: Arc<DashMap<u32, ServiceHandle, fxhash::FxBuildHasher>>,
}

impl ServiceCollection {
    #[inline]
    pub fn get(&self, id: u32) -> Option<ServiceHandle> {
        self.services.get(&id).map(|v| *v)
    }

    pub fn insert(&self, id: u32, handle: ServiceHandle) {
        self.services.insert(id, handle);
    }
}

#[derive(Clone, Copy)]
pub struct ServiceHandle {}

pub async fn spawn_service<E, F>(id: u32, cx: Arc<Context>, executor: E) -> ServiceHandle
where
    E: Fn(ipc_types::Request) -> F + Clone + Send + 'static,
    F: Future<Output = ipc_types::Response> + Send + 'static,
{
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
    let conn_path = ipc_dir.join("conn");
    tokio::spawn(async move {
        // Wait until we have the UDS listener listening.
        permit.notified().await;
        tracing::trace!("Starting the child process for service '{id}'.");
        run_command(format!("service-{id}"), command, kill_notify, conn_path).await;
        tracing::trace!("Exiting service '{id}' execution loop.");
    });

    tokio::spawn(async move {
        run_ctrl_loop(&ipc_dir, cx, cmd_permit, executor).await;
    });

    ServiceHandle {}
}

async fn run_ctrl_loop<E, F>(
    ipc_path: &Path,
    cx: Arc<Context>,
    cmd_permit: Arc<Notify>,
    executor: E,
) where
    E: Fn(ipc_types::Request) -> F + Clone + Send + 'static,
    F: Future<Output = ipc_types::Response> + Send + 'static,
{
    let ctrl_path = ipc_path.join("ctrl");

    // The file might not exist so ignore the error.
    let _ = tokio::fs::remove_file(&ctrl_path).await;

    let listener = UnixListener::bind(ctrl_path).expect("Failed to bind to IPC socket.");
    cmd_permit.notify_one();

    pin! {
        let kill_fut = cx.kill.notified();
    };

    // Every time the service process is restarted (due to failures). The next one will attempt to
    // create a new connection to the same socket and here we will catch it.
    loop {
        tokio::select! {
            biased;
            _ = &mut kill_fut => {
                break;
            },
            Ok((stream, _)) = listener.accept() => {
                let executor = executor.clone();
                let kill = cx.kill.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_stream(stream, kill, executor).await {
                        tracing::error!("Error while handling the unix stream: {e:?}");
                    }
                });
            }
        }
    }

    // We got the kill signal. So let's do some cleanup.
    drop(listener);
}

async fn handle_stream<E, F>(
    stream: UnixStream,
    kill: Arc<Notify>,
    executor: E,
) -> Result<(), Box<dyn Error>>
where
    E: Fn(ipc_types::Request) -> F + 'static,
    F: Future<Output = ipc_types::Response> + Send + 'static,
{
    const IPC_MESSAGE_SIZE: usize = std::mem::size_of::<ipc_types::IpcMessage>();
    const IPC_REQUEST_SIZE: usize = std::mem::size_of::<ipc_types::IpcRequest>();

    pin! {
        let kill_fut = kill.notified();
    }

    let mut read_buffer = [0u8; IPC_REQUEST_SIZE];
    let mut read_pos = 0;
    let mut write_buffer = [0u8; IPC_MESSAGE_SIZE];
    let mut write_pos = IPC_MESSAGE_SIZE;
    let mut task_set = JoinSet::<IpcMessage>::new();
    let mut is_writable = false;
    let mut is_readable = false;

    'outer: loop {
        tracing::trace!("awaiting an event");

        // First check to see if we have something to write to the socket. If so we would be
        // interested in writing to the UnixStream. Otherwise we would only be interested in
        // reading. But would listen for new completed tasks at the same time.

        if write_pos == IPC_MESSAGE_SIZE {
            tokio::select! {
                biased;
                _ = &mut kill_fut => {
                    tracing::trace!("exiting loop");
                    break 'outer;
                },
                Some(msg) = task_set.join_next() => {
                    write_buffer = unsafe { std::mem::transmute(msg) };
                    write_pos = 0;

                    if !is_writable {
                        // Jump back to the beginning of the outer loop. This time message will be
                        // there and we would also be interested in `WRITABLE` status. This will
                        // give us opportunity to turn the `is_writable` flag on.
                        continue 'outer;
                    }
                },
                ready_result = stream.ready(Interest::READABLE) => {
                    let ready = ready_result?;
                    is_readable |= ready.is_readable();
                },
            }
        } else {
            tokio::select! {
                biased;
                _ = &mut kill_fut => {
                    tracing::trace!("exiting loop");
                    break 'outer;
                },
                ready_result = stream.ready(Interest::READABLE) => {
                    let ready = ready_result?;
                    is_writable |= ready.is_writable();
                    is_readable |= ready.is_readable();
                },
            }
        }

        tracing::trace!("is_writable={is_writable},is_readable={is_readable}");

        // While the stream is writable and we have a message to write try to write the message.
        // Only turning the is_writable flag off once we get a WouldBlock error.
        while is_writable && write_pos < IPC_MESSAGE_SIZE {
            match stream.try_write(&write_buffer[write_pos..]) {
                Ok(n) => {
                    write_pos += n;
                },
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    tracing::trace!("is_writable=false");
                    is_writable = false;
                },
                Err(e) => {
                    return Err(e.into());
                },
            }
        }

        while is_readable {
            match stream.try_read(&mut read_buffer[read_pos..]) {
                Ok(0) => {
                    tracing::trace!("Got 0 bytes. Returning.");
                    break 'outer;
                },
                Ok(n) => {
                    read_pos += n;
                },
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    tracing::trace!("is_readable=false");
                    is_readable = false;
                },
                Err(e) => {
                    return Err(e.into());
                },
            }

            if read_pos == IPC_REQUEST_SIZE {
                read_pos = 0;
                let request: ipc_types::IpcRequest =
                    unsafe { std::mem::transmute_copy(&read_buffer) };

                if let Some(ctx) = request.request_ctx {
                    let future = executor(request.request);
                    task_set.spawn(async move {
                        IpcMessage::Response {
                            request_ctx: ctx,
                            response: future.await,
                        }
                    });
                } else {
                    // Only enqueue the request. We don't need to send the response back.
                    tokio::spawn(executor(request.request));
                }
            }
        }
    }

    Ok(())
}

/// Run the given command until the kill signal has been received. Restarting the child on failure.
async fn run_command(
    name: String,
    mut command: Command,
    kill: Arc<Notify>,
    conn_uds_path: PathBuf,
) {
    pin! {
        let kill_fut = kill.notified();
    };

    command
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let mut wait_time = 1000;
    loop {
        // Remove the `/ipc/conn` file before (re-)running
        let _ = tokio::fs::remove_file(&conn_uds_path).await;

        let last_start = Instant::now();
        tracing::trace!("Running command for '{name}");

        let mut child = command.spawn().expect("Command failed to start.");

        select! {
            biased;
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
            biased;
            _ = &mut kill_fut => {
                tracing::trace!("Got the signal to stop '{name}'");
                break;
            }
            _ = tokio::time::sleep(wait_dur) => {
                wait_time = wait_time * 12 / 10;
            }
        }
    }

    tracing::info!("Exiting service execution loop [sid={name}]")
}
