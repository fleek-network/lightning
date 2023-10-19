//! This module contains the implementation of the IPC used by Fleek Network. This is intended
//! for communication between a service (as an standalone process) with the core protocol.
//!
//! To perform this we leverage Unix domain sockets. Our IPC messages are simple struct values
//! that implement `Copy` which means they are safe to be copied byte by byte on the same machine.
//!
//! To standardize this concept a bit more we use `#[repr(C)]` over these IPC messages. Nothing
//! else is done at the current time. This is to be replaced by another serialization format in
//! future, but for now it works for our use cases.

use std::error::Error;
use std::path::PathBuf;

use tokio::io::{self, Interest};
use tokio::net::{UnixListener, UnixStream};

use crate::futures::future_callback;
use crate::ipc_types::{IpcMessage, IpcRequest, Request, Response};

static mut SENDER: Option<tokio::sync::mpsc::Sender<IpcRequest>> = None;
pub(crate) static mut IPC_PATH: Option<PathBuf> = None;
pub(crate) static mut BLOCKSTORE: Option<PathBuf> = None;

/// Bind to the connection stream.
pub async fn conn_bind() -> UnixListener {
    let path = unsafe { IPC_PATH.as_ref() }
        .expect("Service setupt not complete.")
        .join("conn");
    UnixListener::bind(path).expect("IPC bind failed.")
}

/// Init the service event loop using environment variables. This method *MUST* only be called
/// once.
pub fn init_from_env() {
    let blockstore_path = std::env::var("BLOCKSTORE_PATH")
        .expect("Expected BLOCKSTORE_PATH env")
        .try_into()
        .expect("BLOCKSTORE_PATH to be a valid path.");
    let ipc_path = std::env::var("IPC_PATH")
        .expect("Expected IPC_PATH env")
        .try_into()
        .expect("IPC_PATH to be a valid path.");

    tokio::spawn(async {
        let _ = spawn_service_loop(ipc_path, blockstore_path).await;
    });
}

/// Spawn a service with the given connection handler.
pub async fn spawn_service_loop(
    ipc_path: PathBuf,
    blockstore_path: PathBuf,
) -> Result<(), Box<dyn Error>> {
    let ipc_stream = UnixStream::connect(ipc_path.join("ctrl")).await?;
    spawn_service_loop_inner(ipc_stream, ipc_path, blockstore_path).await
}

pub(crate) async fn spawn_service_loop_inner(
    ipc_stream: UnixStream,
    _ipc_path: PathBuf,
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
            // This inner loop will iterate as long as we have messages to write. Once we don't have
            // any message to write. We break from it which will allow us to continue with the code
            // and execute the `read` task, as opposed to just jumping to 'outer' which would prefer
            // pending writes again.
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
                        Ok(0) => {
                            break 'outer;
                        },
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
                handle_message(message);
            }
        }
    }

    Ok(())
}

#[inline]
fn handle_message(message: IpcMessage) {
    match message {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_message_flow() {
        let (s1, s2) = tokio::net::UnixStream::pair().unwrap();
        tokio::spawn(async move {
            let path = std::env::temp_dir().join("test");
            spawn_service_loop_inner(s1, path.clone(), path)
                .await
                .unwrap();
        });

        let started = std::time::Instant::now();
        for i in 0..100 {
            let send_task = tokio::spawn(async move {
                send_and_await_response(Request::QueryClientBalance { pk: [i; 96] }).await
            });

            let mut buffer = [0; std::mem::size_of::<IpcRequest>()];
            let mut pos = 0;
            while pos < buffer.len() {
                s2.readable().await.unwrap();
                match s2.try_read(&mut buffer) {
                    Ok(0) => {
                        break;
                    },
                    Ok(n) => {
                        pos += n;
                    },
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        continue;
                    },
                    Err(e) => {
                        panic!("failed {e:?}");
                    },
                }
            }
            assert_eq!(pos, std::mem::size_of::<IpcRequest>());

            let req = unsafe { std::mem::transmute::<_, IpcRequest>(buffer) };
            assert_eq!(req.request, Request::QueryClientBalance { pk: [i; 96] });

            let result = IpcMessage::Response {
                request_ctx: req.request_ctx.unwrap(),
                response: Response::QueryClientBalance {
                    balance: i as u128 + 100,
                },
            };
            let res_buffer = unsafe {
                std::mem::transmute::<_, [u8; std::mem::size_of::<IpcMessage>()]>(result)
            };
            let mut to_write = res_buffer.as_slice();
            while !to_write.is_empty() {
                s2.writable().await.unwrap();
                match s2.try_write(to_write) {
                    Ok(n) => {
                        to_write = &to_write[n..];
                    },
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        continue;
                    },
                    Err(e) => {
                        panic!("failed {e:?}");
                    },
                }
            }

            let result = send_task.await.unwrap();
            assert_eq!(
                result,
                Response::QueryClientBalance {
                    balance: i as u128 + 100
                }
            );
        }
        let took = started.elapsed();
        println!("took {took:?}");
    }
}
