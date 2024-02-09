use bytes::BytesMut;
use tokio::io::{AsyncRead, AsyncReadExt};

/// A utility function to read a U32 big-endian length delimited payload from an async reader.
///
/// Returns `None` if the stream is exhausted before the promised number of bytes is read.
pub async fn read_length_delimited<R>(reader: &mut R) -> Option<BytesMut>
where
    R: AsyncRead + Unpin,
{
    let mut size = [0; 4];
    // really unnecessary.
    let mut i = 0;
    while i < 4 {
        match reader.read(&mut size[i..]).await {
            Ok(0) | Err(_) => return None,
            Ok(n) => {
                i += n;
            },
        }
    }
    let size = u32::from_be_bytes(size) as usize;
    // now let's read `size` bytes.
    let mut buffer = BytesMut::with_capacity(size);
    while buffer.len() < size {
        match reader.read_buf(&mut buffer).await {
            Ok(0) | Err(_) => return None,
            Ok(_) => {},
        }
    }
    debug_assert_eq!(buffer.len(), size);
    Some(buffer)
}
