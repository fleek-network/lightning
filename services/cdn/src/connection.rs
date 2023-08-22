#![allow(dead_code)]

use std::io::{Error, ErrorKind, Write};

use arrayref::array_ref;
use arrayvec::ArrayVec;
use bytes::BytesMut;
use consts::*;
use fleek_crypto::ClientSignature;
use futures::executor::block_on;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Constant values for the codec.
pub mod consts {
    /// Maximum size for a frame
    pub const MAX_FRAME_SIZE: usize = 66;

    /// [`super::CdnFrame::Request`]
    pub const REQUEST_TAG: u8 = 0x01 << 0;
    /// [`super::CdnFrame::RangeRequest`]
    pub const RANGE_REQUEST_TAG: u8 = 0x01 << 1;
    /// [`super::CdnFrame::ResponseBlock`]
    pub const RESPONSE_BLOCK_TAG: u8 = 0x01 << 2;
    /// [`super::CdnFrame::DeliveryAcknowledgement`]
    pub const DELIVERY_ACK_TAG: u8 = 0x01 << 3;
    /// [`super::CdnFrame::DecryptionKey`]
    pub const DECRYPTION_KEY_TAG: u8 = 0x01 << 4;

    /// The bit flag used for termination signals, to gracefully end a connection with a reason.
    pub const TERMINATION_FLAG: u8 = 0b10000000;

    /// Snappy compression bitmap value
    pub const SNAPPY: u8 = 0x01;
    /// GZip compression bitmap value
    pub const GZIP: u8 = 0x01 << 2;
    /// LZ4 compression bitmap value
    pub const LZ4: u8 = 0x01 << 3;
}

fn is_termination_signal(byte: u8) -> bool {
    byte & TERMINATION_FLAG == TERMINATION_FLAG
}

/// Termination reasons
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    CodecViolation = TERMINATION_FLAG,
    OutOfLanes,
    ServiceNotFound,
    InsufficientBalance,
    Unknown = 0xFF,
}

impl Reason {
    fn from_u8(byte: u8) -> Option<Self> {
        if !is_termination_signal(byte) {
            return None;
        }

        match byte {
            0x80 => Some(Self::CodecViolation),
            0x81 => Some(Self::OutOfLanes),
            0x82 => Some(Self::ServiceNotFound),
            0x83 => Some(Self::InsufficientBalance),
            _ => Some(Self::Unknown),
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceMode {
    Tentative = 0x00,
    Optimistic = 0x01,
}

impl ServiceMode {
    fn from_u8(byte: u8) -> Option<Self> {
        match byte {
            0x00 => Some(Self::Tentative),
            0x01 => Some(Self::Optimistic),
            _ => None,
        }
    }
}

/// Frame tags
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameTag {
    Request = REQUEST_TAG,
    RangeRequest = RANGE_REQUEST_TAG,
    ResponseBlock = RESPONSE_BLOCK_TAG,
    DeliveryAcknowledgement = DELIVERY_ACK_TAG,
    DecryptionKey = DECRYPTION_KEY_TAG,
    TerminationSignal = TERMINATION_FLAG,
}

impl FrameTag {
    #[inline(always)]
    pub fn from_u8(value: u8) -> Option<Self> {
        if is_termination_signal(value) {
            return Some(Self::TerminationSignal);
        }

        match value {
            REQUEST_TAG => Some(Self::Request),
            RANGE_REQUEST_TAG => Some(Self::RangeRequest),
            RESPONSE_BLOCK_TAG => Some(Self::ResponseBlock),
            DELIVERY_ACK_TAG => Some(Self::DeliveryAcknowledgement),
            DECRYPTION_KEY_TAG => Some(Self::DecryptionKey),
            _ => None,
        }
    }

    #[inline(always)]
    pub fn size_hint(&self) -> usize {
        match self {
            FrameTag::Request => 34,
            FrameTag::RangeRequest => 43,
            FrameTag::ResponseBlock => 18,
            FrameTag::DeliveryAcknowledgement => 33,
            FrameTag::DecryptionKey => 34,
            FrameTag::TerminationSignal => 1,
        }
    }
}

/// Frame variants for different requests and responses
///
/// All frames are prefixed with a [`FrameTag`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CdnFrame {
    Request {
        service_mode: ServiceMode,
        hash: [u8; 32],
    },
    RangeRequest {
        hash: [u8; 32],
        start: u64,
        num_blocks: u16,
    },
    /// Node response to confirm resuming a lane.
    ResponseBlock {
        compression: u8,
        bytes_len: u64,
        proof_len: u64,
    },
    /// Raw buffer containing proof or content bytes.
    Buffer(BytesMut),
    /// Client acknowledgment that a block was delivered.
    /// These are batched and submitted by the node for rewards
    DeliveryAcknowledgement { signature: ClientSignature },
    /// Client request to start a service subprotocol
    DecryptionKey { key: [u8; 33] },
    /// Signal from the node the connection was terminated, with a reason.
    TerminationSignal(Reason),
}

impl CdnFrame {
    /// Return the frame's tag or `None` if the frame is a raw buffer.
    #[inline(always)]
    pub fn tag(&self) -> Option<FrameTag> {
        match self {
            Self::Request { .. } => Some(FrameTag::Request),
            Self::RangeRequest { .. } => Some(FrameTag::RangeRequest),
            Self::ResponseBlock { .. } => Some(FrameTag::ResponseBlock),
            Self::DeliveryAcknowledgement { .. } => Some(FrameTag::DeliveryAcknowledgement),
            Self::DecryptionKey { .. } => Some(FrameTag::DecryptionKey),
            Self::TerminationSignal(_) => Some(FrameTag::TerminationSignal),
            // Not a real frame!
            Self::Buffer(_) => None,
        }
    }

    /// Return an estimation of the number of bytes this frame will need.
    #[inline]
    pub fn size_hint(&self) -> usize {
        match self {
            Self::Buffer(buf) => buf.len(),
            _ => self.tag().unwrap().size_hint(),
        }
    }
}

#[derive(Debug)]
pub enum CdnCodecError {
    InvalidNetwork,
    InvalidTag(u8),
    InvalidReason(u8),
    UnexpectedFrame(FrameTag),
    ZeroLengthBlock,
    Io(std::io::Error),
    OccupiedLane,
    Unknown,
}

impl From<std::io::Error> for CdnCodecError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<CdnCodecError> for std::io::Error {
    fn from(value: CdnCodecError) -> Self {
        match value {
            CdnCodecError::Io(e) => e,
            error => Error::new(ErrorKind::Other, format!("{error:?}")),
        }
    }
}

/// Implementation for reading and writing handshake frames on a connection.
pub struct CdnConnection<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> {
    pub reader: R,
    pub writer: W,
    buffer: BytesMut,
    take: usize,
}

impl<R, W> CdnConnection<R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    #[inline(always)]
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            buffer: BytesMut::with_capacity(MAX_FRAME_SIZE),
            take: 0,
        }
    }

    /// Indicate the next `len` bytes are to be returned as a raw buffer. Always called after a
    /// response block, to receive the raw proof and block bytes.
    #[inline(always)]
    pub fn read_buffer(&mut self, len: usize) {
        self.take = len;
        // Ensure we have enough space to read the entire chunk at once if possible.
        self.buffer.reserve(len);
    }

    #[inline(always)]
    pub async fn write_frame(&mut self, frame: CdnFrame) -> std::io::Result<()> {
        match frame {
            CdnFrame::Request { service_mode, hash } => {
                let mut buf = ArrayVec::<u8, 34>::new_const();

                buf.push(FrameTag::Request as u8);
                buf.push(service_mode as u8);
                buf.write_all(&hash).unwrap();

                self.writer.write_all(&buf).await?;
            },
            CdnFrame::RangeRequest {
                hash,
                start,
                num_blocks,
            } => {
                let mut buf = ArrayVec::<u8, 43>::new_const();

                buf.push(FrameTag::RangeRequest as u8);
                buf.write_all(&hash).unwrap();
                buf.write_all(&start.to_be_bytes()).unwrap();
                buf.write_all(&num_blocks.to_be_bytes()).unwrap();

                self.writer.write_all(&buf).await?;
            },
            CdnFrame::ResponseBlock {
                compression,
                bytes_len,
                proof_len,
            } => {
                let mut buf = ArrayVec::<u8, 18>::new_const();

                buf.push(FrameTag::ResponseBlock as u8);
                buf.push(compression);
                buf.write_all(&proof_len.to_be_bytes()).unwrap();
                buf.write_all(&bytes_len.to_be_bytes()).unwrap();

                self.writer.write_all(&buf).await?;
            },
            CdnFrame::Buffer(buf) => {
                self.writer.write_all(&buf).await?;
            },
            CdnFrame::DeliveryAcknowledgement { .. } => {
                let mut buf = ArrayVec::<u8, 33>::new_const();

                buf.push(FrameTag::DeliveryAcknowledgement as u8);
                // TODO: use concrete type for client signature
                buf.write_all(&[0u8; 32]).unwrap();

                self.writer.write_all(&buf).await?;
            },
            CdnFrame::DecryptionKey { key } => {
                let mut buf = ArrayVec::<u8, 34>::new_const();

                buf.push(FrameTag::DecryptionKey as u8);
                buf.write_all(&key).unwrap();

                self.writer.write_all(&buf).await?;
            },
            CdnFrame::TerminationSignal(reason) => {
                self.writer.write_u8(reason as u8).await?;
            },
        }

        Ok(())
    }

    #[inline(always)]
    pub async fn read_frame(&mut self, filter: Option<u8>) -> std::io::Result<Option<CdnFrame>> {
        loop {
            // If we have a full frame, parse and return it.
            if let Some(frame) = self.parse_frame(filter)? {
                return Ok(Some(frame));
            }

            // Otherwise, read as many bytes as we can for a fixed frame.
            if 0 == self.reader.read_buf(&mut self.buffer).await? {
                // Handle connection closed. If there are bytes in the buffer, it means the
                // connection was interrupted mid-transmission.
                if self.buffer.is_empty() {
                    return Ok(None);
                } else {
                    return Err(Error::new(
                        ErrorKind::ConnectionReset,
                        "Client disconnected",
                    ));
                }
            }
        }
    }

    #[inline(always)]
    fn parse_frame(&mut self, filter: Option<u8>) -> std::io::Result<Option<CdnFrame>> {
        let len = self.buffer.len();
        if len == 0 {
            return Ok(None);
        }

        // Should we be reading raw bytes?
        if self.take > 0 {
            return Ok(if len >= self.take {
                // Only return the buffer when we have all the data.
                let bytes = self.buffer.split_to(self.take);
                self.take = 0;
                Some(CdnFrame::Buffer(bytes))
            } else {
                // Otherwise, return none and wait for more bytes.
                None
            });
        }

        let tag_byte = self.buffer[0];

        // Even if the entire frame isn't available, we can already filter and reject invalid
        // frames, terminating connections as soon as possible.
        if let Some(bitmap) = filter {
            if tag_byte & bitmap != tag_byte {
                block_on(async {
                    self.termination_signal(Reason::CodecViolation).await.ok() // We dont care about this result!
                });
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("Invalid tag: {tag_byte}",),
                ));
            }
        }

        // First frame byte is always the tag.
        let tag = match FrameTag::from_u8(self.buffer[0]) {
            Some(tag) => tag,
            None => return Err(Error::new(ErrorKind::InvalidData, "Unknown frame tag")),
        };
        let size_hint = tag.size_hint();

        // If we need more bytes for the frame, return none.
        if len < tag.size_hint() {
            return Ok(None);
        }

        // We're going to take the frame's length, so lets reserve the amount for the next frame.
        self.buffer.reserve(size_hint);

        match tag {
            FrameTag::Request => {
                let buf = self.buffer.split_to(size_hint);
                let service_mode = match ServiceMode::from_u8(buf[1]) {
                    Some(mode) => mode,
                    None => return Err(Error::new(ErrorKind::InvalidData, "Unknown frame tag")),
                };
                let hash = *array_ref!(buf, 2, 32);
                Ok(Some(CdnFrame::Request { service_mode, hash }))
            },
            FrameTag::RangeRequest => {
                let buf = self.buffer.split_to(size_hint);

                let hash = *array_ref!(buf, 1, 32);
                let start = u64::from_be_bytes(*array_ref!(buf, 33, 8));
                let num_blocks = u16::from_be_bytes(*array_ref!(buf, 41, 2));

                Ok(Some(CdnFrame::RangeRequest {
                    hash,
                    start,
                    num_blocks,
                }))
            },
            FrameTag::ResponseBlock => {
                let buf = self.buffer.split_to(size_hint);

                let compression = buf[1];
                let proof_len = u64::from_be_bytes(*array_ref!(buf, 2, 8));
                let bytes_len = u64::from_be_bytes(*array_ref!(buf, 10, 8));

                Ok(Some(CdnFrame::ResponseBlock {
                    compression,
                    bytes_len,
                    proof_len,
                }))
            },
            FrameTag::DeliveryAcknowledgement => {
                let buf = self.buffer.split_to(size_hint);
                let _signature = *array_ref!(buf, 1, 32);

                // TODO: get concrete type for client signature in fleek-crypto
                Ok(Some(CdnFrame::DeliveryAcknowledgement {
                    signature: ClientSignature([0; 48]),
                }))
            },
            FrameTag::DecryptionKey => {
                let buf = self.buffer.split_to(size_hint);
                let key = *array_ref!(buf, 1, 33);

                Ok(Some(CdnFrame::DecryptionKey { key }))
            },
            FrameTag::TerminationSignal => {
                let buf = self.buffer.split_to(size_hint);

                if let Some(reason) = Reason::from_u8(buf[0]) {
                    Ok(Some(CdnFrame::TerminationSignal(reason)))
                } else {
                    Err(CdnCodecError::InvalidReason(buf[0]).into())
                }
            },
        }
    }

    /// Write a termination signal to the stream.
    #[inline(always)]
    pub async fn termination_signal(&mut self, reason: Reason) -> std::io::Result<()> {
        self.write_frame(CdnFrame::TerminationSignal(reason)).await
    }

    /// Finish the connection, consuming the struct and returning the reader and writer.
    pub fn finish(self) -> (R, W) {
        (self.reader, self.writer)
    }
}

#[cfg(test)]
mod tests {
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::mpsc::channel;

    use super::*;

    type TResult = Result<(), CdnCodecError>;

    async fn encode_decode(frame: CdnFrame) -> TResult {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        // accept a single connection
        let (tx, mut rx) = channel(1);
        tokio::task::spawn(async move {
            let (s, _) = listener.accept().await.unwrap();
            tx.send(s).await.unwrap();
        });

        // create streams
        let mut alice_stream = TcpStream::connect(addr).await?;
        let mut bob_stream = rx.recv().await.unwrap();

        // create a cdn connection to encode/decode with
        let (r, w) = alice_stream.split();
        let mut alice = CdnConnection::new(r, w);
        let (r, w) = bob_stream.split();
        let mut bob = CdnConnection::new(r, w);

        // write/read the frame, comparing the result afterwards
        alice.write_frame(frame.clone()).await?;
        let recv_frame = bob.read_frame(None).await?.unwrap();
        assert_eq!(frame, recv_frame);

        Ok(())
    }

    #[tokio::test]
    async fn request() -> TResult {
        encode_decode(CdnFrame::Request {
            service_mode: ServiceMode::Tentative,
            hash: [0u8; 32],
        })
        .await
    }

    #[tokio::test]
    async fn range_request() -> TResult {
        encode_decode(CdnFrame::RangeRequest {
            hash: [0u8; 32],
            start: 0,
            num_blocks: 10,
        })
        .await
    }

    #[tokio::test]
    async fn response_block() -> TResult {
        encode_decode(CdnFrame::ResponseBlock {
            compression: 0,
            proof_len: 0,
            bytes_len: 0,
        })
        .await
    }

    #[tokio::test]
    async fn delivery_ack() -> TResult {
        encode_decode(CdnFrame::DeliveryAcknowledgement {
            signature: ClientSignature([0; 48]),
        })
        .await
    }

    #[tokio::test]
    async fn decryption_key() -> TResult {
        encode_decode(CdnFrame::DecryptionKey { key: [0u8; 33] }).await
    }

    #[tokio::test]
    async fn termination_signal() -> TResult {
        encode_decode(CdnFrame::TerminationSignal(Reason::Unknown)).await
    }
}
