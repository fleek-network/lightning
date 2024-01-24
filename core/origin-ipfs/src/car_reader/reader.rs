use anyhow::{anyhow, Context, Result};
use cid::Cid;
use libipld::cbor::DagCborCodec;
use libipld::codec::Codec;
use tokio::io::{AsyncRead, AsyncReadExt};

use super::utils::Buffer;

const MAX_ALLOC: usize = 4 * 1024 * 1024;

pub struct CarReader<R: AsyncRead + Unpin> {
    reader: R,
    version: u64,
    bytes_read: u64,
    data_read: u64,
    data_size: u64,
    buffer: Vec<u8>,
    roots: Vec<Cid>,
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
    pub characteristics: u128,
    pub data_offset: u64,
    pub data_size: u64,
    pub index_offset: u64,
}

impl<R: AsyncRead + Unpin> CarReader<R> {
    pub async fn new(mut reader: R) -> Result<Self> {
        match header_v1(&mut reader).await {
            Ok((header, bytes_read)) => {
                // This is a car v1
                Ok(Self {
                    reader,
                    version: 1,
                    bytes_read: bytes_read as u64,
                    data_read: 0,
                    data_size: 0, // not available in car v1
                    buffer: vec![0; 1024],
                    roots: header.roots,
                })
            },
            Err(e) => {
                if let Some(Buffer {
                    data,
                    mut bytes_read,
                }) = e.downcast_ref()
                {
                    match pragma_v2(data).await {
                        Ok(_pragma) => {
                            // This is a car v2
                            let header_v2 = header_v2(&mut reader).await?;
                            bytes_read += 40;

                            if header_v2.data_offset > bytes_read as u64 {
                                // For car v2, we potentially need to skip the optional padding
                                let padding_size = header_v2.data_offset as usize - bytes_read;
                                if padding_size > MAX_ALLOC {
                                    // The spec for car v2 does not mention any constrains on the
                                    // size of the
                                    // optional padding, but for safety reasons, we have to bound
                                    // it.
                                    return Err(anyhow!("Padding too large"));
                                }
                                let mut buf = vec![0; padding_size];
                                reader.read_exact(&mut buf).await?;
                            }
                            // After the optional padding, the wrapped car v1 starts
                            let (header_v1, bytes_read) = header_v1(&mut reader).await?;
                            Ok(Self {
                                reader,
                                version: 2,
                                bytes_read: bytes_read as u64,
                                data_read: bytes_read as u64,
                                data_size: header_v2.data_size,
                                buffer: vec![0; 1024],
                                roots: header_v1.roots,
                            })
                        },
                        Err(e) => Err(e),
                    }
                } else {
                    Err(e)
                }
            },
        }
    }

    pub async fn next_block(&mut self) -> Result<Option<(Cid, Vec<u8>)>> {
        if self.version == 2 && self.data_read == self.data_size {
            // For car v2, we have to stop reading content before the index section starts
            return Ok(None);
        }
        match read_block(&mut self.reader, &mut self.buffer).await {
            Ok(Some((cid, data, bytes_read))) => {
                if self.version == 2 {
                    // for car v2, we need to keep track of some state
                    self.bytes_read += bytes_read as u64;
                    self.data_read += bytes_read as u64;
                }
                Ok(Some((cid, data)))
            },
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

/// Adapted from: https://github.com/n0-computer/iroh-car
/// Returns the car v1 header, as well as the number of bytes read.
/// If parsing the car v1 header fails, we return the read bytes in a buffer.
/// In case this is a car v2 file, these bytes make up the pragma.
pub async fn header_v1<R: AsyncRead + Unpin>(reader: &mut R) -> Result<(CarV1Header, usize)> {
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
pub async fn pragma_v2(bytes: &[u8]) -> Result<CarV2Pragma> {
    let pragma: CarV2Pragma = DagCborCodec
        .decode(bytes)
        .map_err(|e| anyhow!("Failed to decode pragma: {e:?}"))?;
    Ok(pragma)
}

/// Returns the car v2 header. The number of bytes is always 40.
pub async fn header_v2<R: AsyncRead + Unpin>(reader: &mut R) -> Result<CarV2Header> {
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
