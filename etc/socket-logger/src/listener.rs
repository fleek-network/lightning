use std::io;
use std::sync::Arc;

use anyhow::Result;
use tokio::io::Interest;
use tokio::net::{UnixListener, UnixStream};
use tui_logger::Drain;

use crate::schema::Record;

pub struct Listener {
    socket: UnixListener,
}

impl Listener {
    pub fn new(socket: UnixListener) -> Self {
        Self { socket }
    }

    pub async fn run(self) -> Result<()> {
        while let Ok((connection, _)) = self.socket.accept().await {
            Connection::new(connection).spawn();
        }
        Ok(())
    }
}

struct Connection {
    drain: Arc<Drain>,
    socket: UnixStream,
}

impl Connection {
    pub fn new(socket: UnixStream) -> Self {
        Self {
            socket,
            drain: Arc::new(Drain::new()),
        }
    }

    pub fn spawn(self) {
        tokio::spawn(async move {
            if let Err(e) = self.run().await {
                eprintln!("unexpected error: {e:?}");
            }
        });
    }

    pub async fn run(&self) -> Result<()> {
        while let Some(data) = self.next().await? {
            let record = bincode::deserialize::<Record>(data.as_slice())?;
            self.drain.log(
                &log::RecordBuilder::new()
                    .level(record.metadata.level.parse()?)
                    .target(record.metadata.target.as_str())
                    .module_path(record.module_path.as_deref())
                    .file(record.file.as_deref())
                    .line(record.line)
                    .args(format_args!("{}", &record.args))
                    .build(),
            );
        }
        Ok(())
    }

    pub async fn next(&self) -> Result<Option<Vec<u8>>> {
        let mut read_buf = vec![0u8; 8];
        let mut bytes_read = 0;
        let mut frame_len = 0;
        loop {
            self.socket.ready(Interest::READABLE).await?;
            // Todo: Remove this inner loop.
            #[allow(clippy::never_loop)]
            'read: loop {
                while frame_len == 0 && bytes_read < 8 {
                    match self.socket.try_read(&mut read_buf[bytes_read..]) {
                        Ok(0) => {
                            return Ok(None);
                        },
                        Ok(n) => {
                            bytes_read += n;
                        },
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            // We received a false positive.
                            break 'read;
                        },
                        Err(e) => {
                            return Err(e.into());
                        },
                    }
                }

                if frame_len == 0 {
                    let bytes: [u8; 8] = read_buf
                        .as_slice()
                        .try_into()
                        .expect("Buffer length is hardcoded");
                    frame_len = usize::from_be_bytes(bytes);
                    // We subtract here to pass entire buffer
                    // to the next loop.
                    read_buf.resize(frame_len - 8, 0);
                    bytes_read = 0;
                }

                while bytes_read < frame_len {
                    match self.socket.try_read(&mut read_buf[bytes_read..]) {
                        Ok(0) => {
                            return Ok(None);
                        },
                        Ok(n) => {
                            bytes_read += n;
                        },
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            // We received a false positive.
                            break 'read;
                        },
                        Err(e) => {
                            return Err(e.into());
                        },
                    }
                }

                return Ok(Some(read_buf));
            }
        }
    }
}
