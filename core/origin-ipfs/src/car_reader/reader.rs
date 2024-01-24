use anyhow::{anyhow, Context, Result};
use cid::Cid;
use libipld::cbor::DagCborCodec;
use libipld::codec::Codec;
use tokio::io::{AsyncRead, AsyncReadExt};

use super::utils::Buffer;

// Taken from: https://github.com/n0-computer/iroh-car
#[derive(Debug, Clone, Default, libipld::DagCbor, PartialEq, Eq)]
pub struct CarV1Header {
    #[ipld]
    pub roots: Vec<Cid>,
    #[ipld]
    pub version: u64,
}

pub struct CarReader<R: AsyncRead + Unpin> {
    reader: R,
}

impl<R: AsyncRead + Unpin> CarReader<R> {
    pub fn new(reader: R) -> Self {
        todo!()
    }

    pub async fn next_block(&mut self) -> Result<Option<(Cid, Vec<u8>)>> {
        todo!()
    }
}

/// Adapted from: https://github.com/n0-computer/iroh-car
/// Returns the car v1 header, as well as the number of bytes read.
/// If parsing the car v1 header fails, we return the read bytes in a buffer.
/// In case this is a car v2 file, these bytes make up the pragma.
async fn header_v1<R: AsyncRead + Unpin>(reader: &mut R) -> Result<(CarV1Header, usize)> {
    let Some((header_size, bytes_read)) = read_varint_usize(reader).await? else {
        return Err(anyhow!("Failed to read var int from header"));
    };
    let mut buf = vec![0; header_size];
    reader.read_exact(&mut buf).await?;
    let header: Result<CarV1Header> = DagCborCodec.decode(&buf);
    let header = match header {
        Ok(header) => header,
        Err(e) => {
            //self.header_buffer = Some(buf);
            let buffer = Buffer {
                data: buf,
                bytes_read: bytes_read + header_size,
            };
            return Err(anyhow!("Failed to decode header: {e:?}")).with_context(|| buffer);
        },
    };

    if header.roots.is_empty() {
        return Err(anyhow!("Car file is empty"));
    }

    // TODO: check version

    Ok((header, bytes_read + header_size))
}

/// Taken mostly from: https://github.com/n0-computer/iroh-car
/// Read a varint from the provided reader. Returns `Ok(None)` on unexpected `EOF`.
async fn read_varint_usize<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<Option<(usize, usize)>, unsigned_varint::io::ReadError> {
    let mut b = unsigned_varint::encode::usize_buffer();
    let mut bytes_read = 0;
    for i in 0..b.len() {
        let n = reader.read(&mut b[i..i + 1]).await?;
        if n == 0 {
            return Ok(None);
        }
        bytes_read += n;
        if unsigned_varint::decode::is_last(b[i]) {
            let slice = &b[..=i];
            let (num, _) = unsigned_varint::decode::usize(slice)?;
            return Ok(Some((num, bytes_read)));
        }
    }
    Err(unsigned_varint::decode::Error::Overflow)?
}
