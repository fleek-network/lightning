use anyhow::{anyhow, Context, Result};
use cid::Cid;
use libipld::cbor::DagCborCodec;
use libipld::codec::Codec;
use tokio::io::{AsyncRead, AsyncReadExt};

use super::utils::Buffer;

const MAX_ALLOC: usize = 4 * 1024 * 1024;

pub struct CarReader<R: AsyncRead + Unpin> {
    reader: R,
}

// Taken from: https://github.com/n0-computer/iroh-car
#[derive(Debug, Clone, Default, libipld::DagCbor, PartialEq, Eq)]
pub struct CarV1Header {
    #[ipld]
    pub roots: Vec<Cid>,
    #[ipld]
    pub version: u64,
}

#[derive(Debug, Clone, Default, libipld::DagCbor, PartialEq, Eq)]
pub struct CarV2Pragma {
    #[ipld]
    pub version: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CarV2Header {
    characteristics: u128,
    data_offset: u64,
    data_size: u64,
    index_offset: u64,
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

/// Returns the car v2 pragma.
async fn pragma_v2(bytes: &[u8]) -> Result<CarV2Pragma> {
    let pragma: CarV2Pragma = DagCborCodec
        .decode(bytes)
        .map_err(|e| anyhow!("Failed to decode pragma: {e:?}"))?;
    Ok(pragma)
}

/// Returns the car v2 header. The number of bytes is always 40.
async fn header_v2<R: AsyncRead + Unpin>(reader: &mut R) -> Result<CarV2Header> {
    let mut buf = vec![0; 16];
    reader.read_exact(&mut buf).await?;
    // These unwraps are all safe because the size of the vecs is static
    // TODO(matthias): make sure that big endian is correct here for a bit field
    let characteristics = u128::from_be_bytes(buf.try_into().unwrap());

    let mut buf = vec![0; 8];
    reader.read_exact(&mut buf).await?;
    let data_offset = u64::from_le_bytes(buf.try_into().unwrap());

    let mut buf = vec![0; 8];
    reader.read_exact(&mut buf).await?;
    let data_size = u64::from_le_bytes(buf.try_into().unwrap());

    let mut buf = vec![0; 8];
    reader.read_exact(&mut buf).await?;
    let index_offset = u64::from_le_bytes(buf.try_into().unwrap());

    let header = CarV2Header {
        characteristics,
        data_offset,
        data_size,
        index_offset,
    };

    Ok(header)
}

/// Mostly taken from: https://github.com/n0-computer/iroh-car
async fn read_block<R>(
    buf_reader: &mut R,
    buf: &mut Vec<u8>,
) -> Result<Option<(Cid, Vec<u8>, usize)>>
where
    R: AsyncRead + Unpin,
{
    if let Some((buf, bytes_read)) = read_block_bytes(buf_reader, buf).await? {
        let mut cursor = std::io::Cursor::new(buf);
        let c = Cid::read_bytes(&mut cursor)?;
        let pos = cursor.position() as usize;

        return Ok(Some((c, buf[pos..].to_vec(), bytes_read)));
    }
    Ok(None)
}

/// Adapted from: https://github.com/n0-computer/iroh-car
async fn read_block_bytes<R>(mut reader: R, buf: &mut Vec<u8>) -> Result<Option<(&[u8], usize)>>
where
    R: AsyncRead + Unpin,
{
    let (length, mut bytes_read): (usize, usize) = match read_varint_usize(&mut reader).await {
        Ok(Some((len, bytes_read))) => (len, bytes_read),
        Ok(None) => return Ok(None),
        Err(e) => {
            return Err(anyhow!("Failed to parse data block: {e:?}"));
        },
    };
    bytes_read += length;
    if length > MAX_ALLOC {
        return Err(anyhow!("Data block too large"));
    }
    if length > buf.len() {
        buf.resize(length, 0);
    }

    reader
        .read_exact(&mut buf[..length])
        .await
        .map_err(|e| anyhow!("Failed to parse data block: {e:?}"))?;

    Ok(Some((&buf[..length], bytes_read)))
}

/// Mostly taken from: https://github.com/n0-computer/iroh-car
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
