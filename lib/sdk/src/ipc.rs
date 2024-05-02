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

use lightning_schema::LightningMessage;
use tokio::io::{self, Interest};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;

use crate::connection::ConnectionListener;
use crate::futures::future_callback;
use crate::ipc_types::{IpcMessage, IpcRequest, Request, Response, DELIMITER_SIZE};

static mut SENDER: Option<tokio::sync::mpsc::Sender<IpcRequest>> = None;
pub(crate) static mut IPC_PATH: Option<PathBuf> = None;
pub(crate) static mut BLOCKSTORE: Option<PathBuf> = None;

/// Bind to the connection stream.
pub async fn conn_bind() -> ConnectionListener {
    let path = unsafe { IPC_PATH.as_ref() }
        .expect("Service setupt not complete.")
        .join("conn");

    let listener = UnixListener::bind(path).expect("IPC bind failed.");
    ConnectionListener::new(listener)
}

/// Init the service event loop using environment variables. This method *MUST* only be called
/// once.
pub fn init_from_env() {
    let blockstore_path: PathBuf = std::env::var("BLOCKSTORE_PATH")
        .expect("Expected BLOCKSTORE_PATH env")
        .into();

    let ipc_path: PathBuf = std::env::var("IPC_PATH")
        .expect("Expected IPC_PATH env")
        .into();

    let (tx, rx) = mpsc::channel::<IpcRequest>(1024);

    // SAFETY: `init_from_env` is the entry function of the entire service process.
    unsafe {
        SENDER = Some(tx);
        BLOCKSTORE = Some(blockstore_path);
        IPC_PATH = Some(ipc_path.clone());
    }

    tokio::spawn(async {
        let _ = spawn_service_loop(ipc_path, rx).await;
    });
}

/// Spawn a service with the given connection handler.
pub async fn spawn_service_loop(
    ipc_path: PathBuf,
    rx: mpsc::Receiver<IpcRequest>,
) -> Result<(), Box<dyn Error>> {
    let ipc_stream = UnixStream::connect(ipc_path.join("ctrl")).await?;
    spawn_service_loop_inner(ipc_stream, rx).await
}

pub(crate) async fn spawn_service_loop_inner(
    ipc_stream: UnixStream,
    mut rx: mpsc::Receiver<IpcRequest>,
) -> Result<(), Box<dyn Error>> {
    // Vecs are mostly fine here because allocation should really only ever occur a few times
    // throught execution runtime since there arent any dynamic types and all of operations never
    // deallocate however we might want to consider using a fixed size

    // IpcRequest
    let mut write_buffer = Vec::<u8>::new();
    let mut write_buffer_pos = 0_usize;
    // IpcMessage
    let mut read_buffer = vec![0; DELIMITER_SIZE];
    let mut read_buffer_pos = 0;

    'outer: loop {
        let ready = if write_buffer.is_empty() {
            tokio::select! {
                ready_result = ipc_stream.ready(Interest::READABLE) => {
                    ready_result?
                },
                Some(req) = rx.recv() => {
                    IpcRequest::encode_length_delimited(&req, &mut write_buffer)?;
                    continue 'outer;
                }
            }
        } else {
            ipc_stream
                .ready(Interest::WRITABLE | Interest::READABLE)
                .await?
        };

        // if we can write and there are messages to write, we should write as many as possible
        if ready.is_writable() && !write_buffer.is_empty() {
            'write: loop {
                while write_buffer_pos < write_buffer.len() {
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

                // always clear because we know it has non zero values
                // doenst deallocate
                write_buffer.clear();
                write_buffer_pos = 0;

                // See if can write any more messages in this loop
                if let Ok(req) = rx.try_recv() {
                    IpcRequest::encode_length_delimited(&req, &mut write_buffer)?;
                } else {
                    break 'write;
                }
            }
        }

        if ready.is_readable() {
            let mut reading_len = true;

            'read: loop {
                // try to read the length from the buffer,
                // if were not already trying to read a message
                while read_buffer_pos < read_buffer.len() {
                    match ipc_stream.try_read(&mut read_buffer[read_buffer_pos..]) {
                        // socket reset
                        Ok(0) => {
                            tracing::warn!("service control loop connection reset");
                            break 'outer;
                        },
                        Ok(n) => {
                            read_buffer_pos += n;
                            // continue
                        },
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            // there is nothiing in the stream to read
                            break 'read;
                        },
                        Err(e) => {
                            return Err(e.into());
                        },
                    }
                }

                // if the len is 0 then we dont have any messages yet, therefore we jsut finished
                // reading the len or else we would have broken out
                if reading_len {
                    // assume were running on a 64bit machine
                    let read_len = usize::from_le_bytes(
                        read_buffer[..8]
                            .try_into()
                            .expect("can create len 8 buffer from read buffer"),
                    );

                    // were now using this as the postion in the message buffer
                    read_buffer_pos = 0;

                    // resize will not deallocate
                    read_buffer.resize(read_len, 0);
                    reading_len = false;

                    continue 'read;
                }

                let message = IpcMessage::decode(&read_buffer)?;

                // cleanup now that weve read the message
                read_buffer_pos = 0;
                read_buffer.resize(DELIMITER_SIZE, 0);
                reading_len = true;

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
        let (tx, rx) = mpsc::channel::<IpcRequest>(1024);
        unsafe {
            SENDER = Some(tx);
        }
        let (s1, s2) = tokio::net::UnixStream::pair().unwrap();
        tokio::spawn(async move {
            spawn_service_loop_inner(s1, rx).await.unwrap();
        });

        let started = std::time::Instant::now();
        for i in 0..100 {
            let send_task = tokio::spawn(async move {
                send_and_await_response(Request::QueryClientBandwidth { pk: [i; 96].into() }).await
            });

            let mut len_buffer = [0_u8; 8];
            let mut buffer = Vec::new();
            let mut pos = 0;
            while pos < 8 {
                s2.readable().await.unwrap();
                match s2.try_read(&mut len_buffer[pos..]) {
                    Ok(0) => {
                        panic!("connection reset");
                    },
                    Ok(n) => {
                        println!("read {n} bytes");
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

            let len = usize::from_le_bytes(len_buffer);
            pos = 0;

            while pos < len {
                s2.readable().await.unwrap();
                match s2.try_read_buf(&mut buffer) {
                    Ok(0) => {
                        panic!("connection reset");
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
            let req = IpcRequest::decode(&buffer).unwrap();

            assert_eq!(
                req.request,
                Request::QueryClientBandwidth { pk: [i; 96].into() }
            );

            let result = IpcMessage::Response {
                request_ctx: req.request_ctx.unwrap(),
                response: Response::QueryClientBandwidth {
                    balance: i as u128 + 100,
                },
            };
            let mut res_buffer = Vec::new();
            result.encode_length_delimited(&mut res_buffer).unwrap();

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
                Response::QueryClientBandwidth {
                    balance: i as u128 + 100
                }
            );
        }
        let took = started.elapsed();
        println!("took {took:?}");
    }
}
