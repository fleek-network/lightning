use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use fn_sdk::header::read_header;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::pin;
use tokio::sync::Notify;
use tokio::task::JoinSet;

/// This worker listens to requests from the handshake, performs the necessary work (for example
/// fetching a file), and then notifies the enclave.
pub struct Worker<P: AsRef<Path>> {
    handshake_socket_path: P,
    enclave_socket_path: P,
    shutdown: Arc<Notify>,
}

impl<P: AsRef<Path>> Worker<P> {
    pub fn new(handshake_socket_path: P, enclave_socket_path: P, shutdown: Arc<Notify>) -> Self {
        Self {
            handshake_socket_path,
            enclave_socket_path,
            shutdown,
        }
    }

    pub async fn start(self) -> Result<()> {
        let listener = UnixListener::bind(self.handshake_socket_path)?;
        let mut set = JoinSet::new();

        let shutdown_fut = self.shutdown.notified();
        pin!(shutdown_fut);
        loop {
            tokio::select! {
                res = listener.accept() => {
                    match res {
                        Ok((stream, _addr)) => {
                            let task = handle_request(stream, self.enclave_socket_path.as_ref().to_path_buf());
                            set.spawn(task);
                        },
                        Err(e) => {
                            println!("failed to accept connection via unix stream: {e:?}");
                        },
                    }
                }
                Some(task) = set.join_next() => {
                    let msg = task.unwrap().unwrap();
                }
                _ = &mut shutdown_fut => {
                    break;
                }
            }
        }
        Ok(())
    }
}

async fn handle_request(mut stream: UnixStream, enclave_socket_path: PathBuf) -> Result<()> {
    // Get request from handshake

    let header = read_header(&mut stream).await.context("failed to read header from unix stream")?;
    println!("received request: {header:?}");

    // Do the work to handle the request

    // Notify the enclave

    let mut enclave_stream = UnixStream::connect(enclave_socket_path).await?;
    enclave_stream.write_all(&32u32.to_be_bytes()).await?;
    enclave_stream.write_all(&[1; 32]).await?;

    Ok(())
}

#[derive(Debug)]
struct Message {
    id: u32,
    content: String,
}

impl From<Message> for Vec<u8> {
    fn from(value: Message) -> Self {
        let id_bytes = value.id.to_be_bytes();
        let content_bytes = value.content.as_bytes();
        let length = content_bytes.len() as u32;
        let length_bytes = length.to_be_bytes();
        let mut bytes =
            Vec::with_capacity(id_bytes.len() + length_bytes.len() + content_bytes.len());
        bytes.extend(id_bytes);
        bytes.extend(length_bytes);
        bytes.extend(content_bytes);
        bytes
    }
}

impl TryFrom<&[u8]> for Message {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let mut index = 4;
        let id_bytes: [u8; 4] = value
            .get(0..index)
            .context("out of bounds")?
            .try_into()
            .unwrap();
        let id = u32::from_be_bytes(id_bytes);
        let length_bytes: [u8; 4] = value
            .get(index..index + 4)
            .context("out of bounds")?
            .try_into()
            .unwrap();
        index += 4;
        let length = u32::from_be_bytes(length_bytes) as usize;
        let content_bytes = value.get(index..index + length).context("out of bounds")?;
        let content = String::from_utf8(content_bytes.to_vec())?;
        Ok(Self { id, content })
    }
}

#[cfg(test)]
mod tests {
    use std::env::temp_dir;
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::io::AsyncWriteExt;
    use tokio::net::UnixStream;
    use tokio::sync::Notify;

    use super::Worker;
    use crate::worker::Message;

    #[tokio::test]
    async fn test_basic() {
        let handshake_socket_path = temp_dir().join("handshake_service_pod_test");
        let enclave_socket_path = temp_dir().join("enclave_service_pod_test");

        if handshake_socket_path.exists() {
            std::fs::remove_file(&handshake_socket_path).unwrap();
        }
        if enclave_socket_path.exists() {
            std::fs::remove_file(&enclave_socket_path).unwrap();
        }

        let handshake_socket_path_clone = handshake_socket_path.clone();
        let sender_fut = async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let mut stream = UnixStream::connect(handshake_socket_path_clone).await.unwrap();
            let msg = Message {
                id: 1,
                content: String::from("abc"),
            };
            let bytes: Vec<u8> = msg.into();
            let length = bytes.len() as u32;
            stream.write_all(&length.to_be_bytes()).await.unwrap();
            stream.write_all(&bytes).await.unwrap();
        };

        let recv_fut = async move {
            let shutdown = Arc::new(Notify::new());
            let worker = Worker::new(handshake_socket_path, enclave_socket_path, shutdown.clone());
            worker.start().await.unwrap();
        };

        futures::join!(sender_fut, recv_fut);
    }
}
