use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use dashmap::DashMap;
use fleek_crypto::{ClientPublicKey, NodePublicKey};
use fn_sdk::ipc_types::{self, IpcMessage, IpcRequest, DELIMITER_SIZE};
use lightning_interfaces::prelude::*;
use lightning_interfaces::schema::task_broker::TaskScope;
use lightning_utils::application::QueryRunnerExt;
use rand::seq::SliceRandom;
use rand::thread_rng;
use tokio::io::{self, Interest};
use tokio::net::{UnixListener, UnixStream};
use tokio::process::Command;
use tokio::sync::Notify;
use tokio::task::JoinSet;
use tokio::{pin, select};
use tracing::instrument;
use triomphe::Arc;

/// The shared object with every service.
pub struct Context<C: Collection> {
    pub blockstore_path: PathBuf,
    pub ipc_path: PathBuf,
    pub fetcher_socket: FetcherSocket,
    pub query_runner: c!(C::ApplicationInterface::SyncExecutor),
    pub task_broker: C::TaskBrokerInterface,
    pub our_public_key: NodePublicKey,
}

impl<C: Collection> Context<C> {
    pub async fn run(&self, request: ipc_types::Request) -> ipc_types::Response {
        match request {
            ipc_types::Request::QueryClientBandwidth { pk } => {
                let balance = self
                    .query_runner
                    .client_key_to_account_key(&ClientPublicKey(pk.into()))
                    .and_then(|address| {
                        self.query_runner
                            .get_account_info(&address, |a| a.bandwidth_balance)
                    })
                    .unwrap_or(0);
                ipc_types::Response::QueryClientBandwidth { balance }
            },
            ipc_types::Request::QueryClientFLK { pk } => {
                let balance = self
                    .query_runner
                    .client_key_to_account_key(&ClientPublicKey(pk.into()))
                    .and_then(|address| {
                        self.query_runner
                            .get_account_info(&address, |a| a.flk_balance)
                    })
                    .unwrap_or(0_u32.into());
                ipc_types::Response::QueryClientFLK {
                    balance: balance.try_into().unwrap(),
                }
            },
            ipc_types::Request::FetchFromOrigin { origin, uri } => {
                let hash = match self
                    .fetcher_socket
                    .run(lightning_interfaces::types::FetcherRequest::Put {
                        pointer: lightning_interfaces::types::ImmutablePointer {
                            origin: match origin {
                                0 => lightning_interfaces::types::OriginProvider::IPFS,
                                1 => lightning_interfaces::types::OriginProvider::HTTP,
                                _ => unreachable!(),
                            },
                            uri,
                        },
                    })
                    .await
                    .unwrap()
                {
                    lightning_interfaces::types::FetcherResponse::Put(hash) => hash.ok(),
                    lightning_interfaces::types::FetcherResponse::Fetch(_) => unreachable!(),
                };

                ipc_types::Response::FetchFromOrigin { hash }
            },
            ipc_types::Request::FetchBlake3 { hash } => {
                let succeeded = match self
                    .fetcher_socket
                    .run(lightning_interfaces::types::FetcherRequest::Fetch { hash })
                    .await
                    .unwrap()
                {
                    lightning_interfaces::types::FetcherResponse::Put(_) => unreachable!(),
                    lightning_interfaces::types::FetcherResponse::Fetch(v) => v.is_ok(),
                };
                ipc_types::Response::FetchBlake3 { succeeded }
            },
            ipc_types::Request::Task {
                depth,
                scope,
                service,
                payload,
            } => {
                let (responses, signatures): (Vec<_>, Vec<_>) = self
                    .task_broker
                    .run(
                        depth,
                        match scope {
                            0 => TaskScope::Local,
                            1 => TaskScope::Single,
                            2.. => TaskScope::Cluster,
                        },
                        schema::task_broker::TaskRequest {
                            service,
                            timestamp: SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                            payload: payload.into(),
                        },
                    )
                    .await
                    .into_iter()
                    .filter_map(|res| res.ok().map(|v| (v.payload.to_vec(), [0u8; 64])))
                    .unzip();

                ipc_types::Response::Task {
                    responses,
                    signatures,
                }
            },
            ipc_types::Request::FetchPeerIps { amount } => {
                let mut peers = self.query_runner.get_active_nodes();
                let mut rng = thread_rng();
                peers.shuffle(&mut rng);
                let peer_ips = peers
                    .iter()
                    .take(amount as usize)
                    .map(|v| v.info.domain.to_string())
                    .collect();
                ipc_types::Response::FetchPeerIps { peer_ips }
            },
            ipc_types::Request::FetchNodeIndex {} => {
                let node_index = self.query_runner.pubkey_to_index(&self.our_public_key);
                ipc_types::Response::FetchNodeIndex { node_index }
            },
            _ => unreachable!(),
        }
    }
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

    #[inline]
    pub fn insert(&self, id: u32, handle: ServiceHandle) {
        self.services.insert(id, handle);
    }
}

#[derive(Clone, Copy)]
pub struct ServiceHandle {}

#[allow(unused)]
pub async fn spawn_service<C: Collection>(
    id: u32,
    cx: Arc<Context<C>>,
    waiter: ShutdownWaiter,
) -> ServiceHandle {
    tracing::info!("Initializing service {id}");

    let ipc_dir = cx.ipc_path.join(format!("service-{id}"));

    // First try to remove the directory if it does exits. And then create it
    // again. Ignoring the errors that may occur while removing.
    let _ = tokio::fs::remove_dir_all(&ipc_dir).await;
    tokio::fs::create_dir_all(&ipc_dir)
        .await
        .expect("Failed to create IPC directory for service.");

    let mut cmd = match which::which(format!("fn-service-{id}")) {
        // Use the standalone service binary
        Ok(path) => Command::new(path),
        Err(_) => {
            // Otherwise, relaunch the current binary for running statically linked services
            let mut args = std::env::args_os();
            let program = args.next().unwrap();
            let mut cmd = Command::new(program);
            cmd.args(args);
            cmd
        },
    };

    cmd.env("SERVICE_ID", format!("{id}"))
        .env("BLOCKSTORE_PATH", &cx.blockstore_path)
        .env("IPC_PATH", &ipc_dir);

    panic_report::add_context(format!("service_{id}"), format!("{cmd:?}"));

    let cmd_permit = Arc::new(Notify::new());
    let permit = cmd_permit.clone();
    let conn_path = ipc_dir.join("conn");

    #[cfg(not(test))]
    {
        let waiter = waiter.clone();
        tokio::spawn(async move {
            // Wait until we have the UDS listener listening.
            permit.notified().await;
            tracing::trace!("Starting the child process for service '{id}'.");
            run_command(format!("service-{id}"), cmd, waiter, conn_path).await;
            tracing::trace!("Exiting service '{id}' execution loop.");
        });
    }

    spawn!(
        async move {
            let waiter2 = waiter.clone();
            waiter
                .run_until_shutdown(async move {
                    run_ctrl_loop(&ipc_dir, cx, cmd_permit, waiter2).await;
                })
                .await;
        },
        "SERVICE-EXECUTOR: shutdown waiter"
    );

    ServiceHandle {}
}

async fn run_ctrl_loop<C: Collection>(
    ipc_path: &Path,
    ctx: Arc<Context<C>>,
    cmd_permit: Arc<Notify>,
    waiter: ShutdownWaiter,
) {
    let ctrl_path = ipc_path.join("ctrl");

    // The file might not exist so ignore the error.
    let _ = tokio::fs::remove_file(&ctrl_path).await;

    let listener = UnixListener::bind(ctrl_path).expect("Failed to bind to IPC socket.");
    cmd_permit.notify_one();

    // Every time the service process is restarted (due to failures). The next one will attempt to
    // create a new connection to the same socket and here we will catch it.
    while let Ok((stream, _)) = listener.accept().await {
        // spawn a new task to handle the stream
        let ctx = ctx.clone();
        let waiter = waiter.clone();
        spawn!(
            async move {
                waiter
                    .run_until_shutdown(async move {
                        if let Err(e) = handle_stream(stream, ctx).await {
                            tracing::error!("Error while handling the unix stream: {e:?}");
                        }
                    })
                    .await
            },
            "SERVICE-EXECUTOR: ctrl loop"
        );
    }
}

#[instrument(skip(stream, ctx))]
async fn handle_stream<C: Collection>(
    stream: UnixStream,
    ctx: Arc<Context<C>>,
) -> Result<(), Box<dyn Error>> {
    // incoming IpcRequests
    // start with a buffer of 8 bytes to read the length delimiter
    let mut read_buffer = vec![0; DELIMITER_SIZE];
    let mut read_buffer_pos = 0;

    // outgoing IpcMessages
    let mut write_buffer = Vec::<u8>::new();
    let mut write_buffer_pos = 0;

    let mut task_set = JoinSet::<IpcMessage>::new();

    'outer: loop {
        // Theres no messages to write
        let ready = if write_buffer.is_empty() {
            tokio::select! {
                Some(msg) = task_set.join_next() => {
                    match msg {
                        Ok(msg) => {
                            IpcMessage::encode_length_delimited(&msg, &mut write_buffer)?;
                            continue 'outer;
                        },
                        Err(e) => {
                            tracing::error!("Error while processing task: {e:?}");
                            return Err(e.into());
                        },
                    }
                },
                ready_result = stream.ready(Interest::READABLE) => {
                    ready_result?
                },
            }
        } else {
            stream
                .ready(Interest::READABLE | Interest::WRITABLE)
                .await?
        };

        // dont try to write more than 1 response because the (async) tasks take a while we
        // want to get them started asap
        if ready.is_writable() && !write_buffer.is_empty() {
            while write_buffer_pos < write_buffer.len() {
                match stream.try_write(&write_buffer[write_buffer_pos..]) {
                    Ok(n) => {
                        write_buffer_pos += n;
                    },
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        break;
                    },
                    Err(e) => {
                        return Err(e.into());
                    },
                }
            }

            if write_buffer_pos == write_buffer.len() {
                // we've written everything
                write_buffer.clear();
                write_buffer_pos = 0;
            }
        }

        // these might take awhile so we want to get them kicked off asap, if theres
        // multiple messages in the stream then this will handle them all
        // before moving on
        if ready.is_readable() {
            let mut reading_len = true;

            'read: loop {
                // try to read the length delimiter first
                while read_buffer_pos < read_buffer.len() {
                    match stream.try_read(&mut read_buffer[read_buffer_pos..]) {
                        Ok(0) => {
                            tracing::warn!("Connection reset control loop");
                            break 'outer;
                        },
                        Ok(n) => {
                            read_buffer_pos += n;
                        },
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            break 'read;
                        },
                        Err(e) => {
                            return Err(e.into());
                        },
                    }
                }

                // Check if were only reading the length delimiter
                if reading_len {
                    let read_len = usize::from_le_bytes(
                        read_buffer[..8]
                            .try_into()
                            .expect("Can create len 4 array from read buffer slice"),
                    );

                    // here we just resize expected size buffer to the correct size and reset the
                    // position
                    read_buffer_pos = 0;
                    read_buffer.resize(read_len, 0);

                    // make sure we dont hit this block again
                    reading_len = false;

                    // get more bytes
                    continue 'read;
                }

                let request = IpcRequest::decode(&read_buffer)?;

                // reset the buffer back to 8 bytes for the next delimiter
                read_buffer.resize(DELIMITER_SIZE, 0);
                read_buffer_pos = 0;
                reading_len = true;

                if let Some(request_ctx) = request.request_ctx {
                    let ctx = ctx.clone();
                    task_set.spawn(async move {
                        let response = ctx.run(request.request).await;
                        IpcMessage::Response {
                            request_ctx,
                            response,
                        }
                    });
                } else {
                    // Only enqueue the request. We don't need to send the response back.
                    let ctx = ctx.clone();
                    spawn!(
                        async move {
                            ctx.run(request.request).await;
                        },
                        "SERVICE-EXECUTOR: run request"
                    );
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
    kill: ShutdownWaiter,
    conn_uds_path: PathBuf,
) {
    pin! {
        let kill_fut = kill.wait_for_shutdown();
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
        tracing::debug!("Starting child process '{name}' with {command:?}");

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
                    tracing::info!("Last run seemed to have been healthy. Waiting for 100ms.");
                    // Restart the wait time.
                    wait_time = 100;
                } else if wait_time <= 100 {
                    wait_time = 1000;
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
