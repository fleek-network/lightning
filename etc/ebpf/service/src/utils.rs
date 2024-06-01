use std::io;

use bytes::Bytes;
use tokio::io::Interest;
use tokio::net::UnixStream;

pub async fn write(socket: &UnixStream, bytes: Bytes) -> io::Result<()> {
    let mut bytes_to_write = 0;
    loop {
        socket.ready(Interest::WRITABLE).await?;
        'write: while bytes_to_write < bytes.len() {
            match socket.try_write(&bytes[bytes_to_write..]) {
                Ok(n) => {
                    bytes_to_write += n;
                },
                Err(e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
                    // We received a false positive.
                    break 'write;
                },
                Err(e) => {
                    return Err(e);
                },
            }
        }
    }
}

pub async fn read(socket: &UnixStream) -> io::Result<Option<Bytes>> {
    let mut read_buf = vec![0u8; 8];
    let mut bytes_read = 0;
    let mut frame_len = 0;
    loop {
        socket.ready(Interest::READABLE).await?;
        // Todo: address this.
        #[allow(clippy::never_loop)]
        'read: loop {
            while frame_len == 0 && bytes_read < 8 {
                match socket.try_read(&mut read_buf[bytes_read..]) {
                    Ok(0) => {
                        return Ok(None);
                    },
                    Ok(n) => {
                        bytes_read += n;
                    },
                    Err(e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
                        // We received a false positive.
                        break 'read;
                    },
                    Err(e) => {
                        return Err(e);
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
                // to EbpfServiceFrame deserializer further below.
                read_buf.resize(frame_len - 8, 0);
                bytes_read = 0;
            }

            while bytes_read < frame_len {
                match socket.try_read(&mut read_buf[bytes_read..]) {
                    Ok(0) => {
                        return Ok(None);
                    },
                    Ok(n) => {
                        bytes_read += n;
                    },
                    Err(e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
                        // We received a false positive.
                        break 'read;
                    },
                    Err(e) => {
                        return Err(e);
                    },
                }
            }

            return Ok(Some(read_buf.into()));
        }
    }
}

#[cfg(feature = "server")]
pub async fn read_service_header(socket: &UnixStream) -> io::Result<Option<u32>> {
    let mut read_buf = vec![0u8; 4];
    let mut bytes_read = 0;
    loop {
        socket.ready(Interest::READABLE).await?;
        'read: loop {
            while bytes_read < 4 {
                match socket.try_read(&mut read_buf[bytes_read..]) {
                    Ok(0) => {
                        return Ok(None);
                    },
                    Ok(n) => {
                        bytes_read += n;
                    },
                    Err(e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
                        // We received a false positive.
                        break 'read;
                    },
                    Err(e) => {
                        return Err(e.into());
                    },
                }
            }
            let bytes: [u8; 4] = read_buf.try_into().expect("Buffer length is hardcoded");
            return Ok(Some(u32::from_le_bytes(bytes)));
        }
    }
}
