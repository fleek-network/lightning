use std::error::Error;
use std::path::{Path, PathBuf};

use fleek_crypto::ClientPublicKey;
use tokio::io::{self, Interest};
use tokio::net::UnixStream;

use crate::futures::future_callback;
use crate::ipc_types::{IpcMessage, IpcRequest, Request, Response};

static mut SENDER: Option<tokio::sync::mpsc::Sender<IpcRequest>> = None;
pub(crate) static mut BLOCKSTORE: Option<PathBuf> = None;

pub trait ServiceHandlers {
    /// Handle the new connection.
    fn connected(socket: UnixStream, pk: ClientPublicKey);
}

/// Spawn a service with the given connection handler.
pub async fn spawn_service_loop<H: ServiceHandlers>(
    ipc_path: PathBuf,
    blockstore_path: PathBuf,
) -> Result<(), Box<dyn Error>> {
    let ipc_stream = UnixStream::connect(ipc_path.join("socket")).await?;
    spawn_service_loop_inner::<H>(ipc_stream, ipc_path, blockstore_path).await
}

pub(crate) async fn spawn_service_loop_inner<H: ServiceHandlers>(
    ipc_stream: UnixStream,
    ipc_path: PathBuf,
    blockstore_path: PathBuf,
) -> Result<(), Box<dyn Error>> {
    const IPC_REQUEST_SIZE: usize = std::mem::size_of::<IpcRequest>();
    const IPC_MESSAGE_SIZE: usize = std::mem::size_of::<IpcMessage>();

    let (tx, mut rx) = tokio::sync::mpsc::channel::<IpcRequest>(1024);

    // SAFETY: `spawn_service_loop` is the entry function of the entire service process.
    unsafe {
        SENDER = Some(tx);
        BLOCKSTORE = Some(blockstore_path);
    }

    // IpcRequest
    let mut write_buffer = [0; IPC_REQUEST_SIZE];
    let mut write_buffer_pos = IPC_REQUEST_SIZE;
    // IpcMessage
    let mut read_buffer = [0; IPC_MESSAGE_SIZE];
    let mut read_buffer_pos = 0;

    'outer: loop {
        // Check to see if we have something to write. If the current `write_buffer`
        // is already flushed out (write_buffer_pos == IPC_REQUEST_SIZE), try to get
        // something from the mpsc channel and encode it.

        if write_buffer_pos == IPC_REQUEST_SIZE {
            if let Ok(req) = rx.try_recv() {
                write_buffer_pos = 0;
                write_buffer =
                    unsafe { std::mem::transmute::<IpcRequest, [u8; IPC_REQUEST_SIZE]>(req) };
            }
        }

        let ready = if write_buffer_pos == IPC_REQUEST_SIZE {
            // We're not interested to write anything atm. So we have to wait until the UDS
            // is ready for read. And we can also at the same time wait until there is message
            // in the mpsc channel.
            tokio::select! {
                ready_result = ipc_stream.ready(Interest::READABLE) => {
                    ready_result
                },
                Some(req) = rx.recv() => {
                    write_buffer_pos = 0;
                    write_buffer =
                        unsafe { std::mem::transmute::<IpcRequest, [u8; IPC_REQUEST_SIZE]>(req) };
                    // Okay now we have something to write. Let's jump back to the beginning of
                    // the outer loop. This time `write_buffer_pos == 0` and that would trigger
                    // us waiting for `ready(WRITABLE)` and the rest will follow as normal.
                    continue 'outer;
                }
            }
        } else {
            ipc_stream
                .ready(Interest::WRITABLE | Interest::READABLE)
                .await
        }?;

        if ready.is_writable() {
            // This inner loop will iterate as long as we have messages to write. Once we don't
            // have any message to write. we break from which will allow us to continue with the
            // code and execute the `read` task. As opposed to just jumping to 'outer' which would
            // prefer write again.
            'write: loop {
                // The socket is writable. First try to flush whatever that is left in the write
                // buffer.
                while write_buffer_pos < IPC_REQUEST_SIZE {
                    match ipc_stream.try_write(&write_buffer[write_buffer_pos..]) {
                        Ok(n) => {
                            write_buffer_pos += n;
                        },
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            break 'write;
                        },
                        Err(e) => {
                            return Err(e.into());
                        },
                    }
                }

                if let Ok(req) = rx.try_recv() {
                    write_buffer_pos = 0;
                    write_buffer =
                        unsafe { std::mem::transmute::<IpcRequest, [u8; IPC_REQUEST_SIZE]>(req) };
                } else {
                    break 'write;
                }
            }
        }

        if ready.is_readable() {
            'read: loop {
                while read_buffer_pos < IPC_MESSAGE_SIZE {
                    match ipc_stream.try_read(&mut read_buffer[read_buffer_pos..]) {
                        Ok(n) => {
                            read_buffer_pos += n;
                        },
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            break 'read;
                        },
                        Err(e) => {
                            return Err(e.into());
                        },
                    }
                }

                debug_assert_eq!(read_buffer_pos, IPC_MESSAGE_SIZE);
                read_buffer_pos = 0;
                let message = unsafe {
                    std::mem::transmute_copy::<[u8; IPC_MESSAGE_SIZE], IpcMessage>(&read_buffer)
                };
                handle_message::<H>(&ipc_path, message);
            }
        }
    }
}

#[inline]
fn handle_message<H: ServiceHandlers>(ipc_dir: &Path, message: IpcMessage) {
    match message {
        IpcMessage::Connected {
            connection_id,
            client,
        } => {
            let path = ipc_dir.join(format!("{}.socks", connection_id));
            tokio::spawn(async move {
                let socket = UnixStream::connect(path).await.unwrap();
                let pk = ClientPublicKey(client);
                H::connected(socket, pk);
            });
        },
        IpcMessage::Response {
            request_ctx,
            response,
        } => {
            // Wake up the future that is awaiting for the response.
            future_callback(request_ctx.into(), response);
        },
    }
}

/// Raw API to send a request to the core via the IPC without awaiting the response.
///
/// # Panics
///
/// You should only call this method from within a service handlers.
pub async fn send_no_response(request: Request) {
    unsafe {
        let sender = SENDER.as_ref().expect("setup not completed");
        sender
            .send(IpcRequest {
                request_ctx: None,
                request,
            })
            .await
            .expect("Failed to send the IPC message.");
    }
}

/// Raw API to send a request to the core via the IPC which returns a future that will be resolved
/// with the response.
///
/// # Panics
///
/// You should only call this method from within a service handlers.
pub async fn send_and_await_response(request: Request) -> Response {
    let (request_ctx, future) = crate::futures::create_future();
    unsafe {
        let sender = SENDER.as_ref().expect("setup not completed");
        sender
            .send(IpcRequest {
                request_ctx: Some(request_ctx.into()),
                request,
            })
            .await
            .expect("Failed to send the IPC message.");
    }
    future.await
}
