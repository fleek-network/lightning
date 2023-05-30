#![allow(dead_code)]

use std::io::{Error, ErrorKind, Write};

use arrayref::array_ref;
use arrayvec::ArrayVec;
use bytes::BytesMut;
use consts::*;
use futures::executor::block_on;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::types::{BlsPublicKey, BlsSignature, Hash, Nonce, Secp256k1PublicKey};

/// Constant values for the codec.
pub mod consts {
    /// Network byte prefix in [`super::UrsaFrame::HandshakeRequest`]
    pub const NETWORK: [u8; 5] = *b"DRACO";
    /// Maximum size for a frame
    pub const MAX_FRAME_SIZE: usize = 1024;

    /// [`super::HandshakeFrame::HandshakeRequest`]
    pub const HANDSHAKE_REQ_TAG: u8 = 0x01 << 0;
    /// [`super::HandshakeFrame::HandshakeResponse`]
    pub const HANDSHAKE_RES_TAG: u8 = 0x01 << 1;
    /// [`super::HandshakeFrame::HandshakeResponseUnlock`]
    pub const HANDSHAKE_RES_UNLOCK_TAG: u8 = 0x01 << 2;
    /// [`super::HandshakeFrame::DeliveryAcknowledgement`]
    pub const DELIVERY_ACK_TAG: u8 = 0x01 << 3;
    /// [`super::UrsaFrame::ServiceRequest`]
    pub const SERVICE_REQ_TAG: u8 = 0x01 << 4;
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

#[test]
fn term() {
    assert!(is_termination_signal(0xFF))
}

/// Termination reasons
#[repr(u8)]
#[derive(Clone, Debug, PartialEq, Eq)]
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

/// Last known data for a lane
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LastLaneData {
    pub bytes: u64,
    pub signature: BlsSignature,
}

/// Frame tags
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameTag {
    HandshakeRequest = HANDSHAKE_REQ_TAG,
    HandshakeResponse = HANDSHAKE_RES_TAG,
    HandshakeResponseUnlock = HANDSHAKE_RES_UNLOCK_TAG,
    DeliveryAcknowledgement = DELIVERY_ACK_TAG,
    ServiceRequest = SERVICE_REQ_TAG,
    TerminationSignal = TERMINATION_FLAG,
}

impl FrameTag {
    #[inline(always)]
    pub fn from_u8(value: u8) -> Option<Self> {
        if is_termination_signal(value) {
            return Some(Self::TerminationSignal);
        }

        match value {
            HANDSHAKE_REQ_TAG => Some(Self::HandshakeRequest),
            HANDSHAKE_RES_TAG => Some(Self::HandshakeResponse),
            HANDSHAKE_RES_UNLOCK_TAG => Some(Self::HandshakeResponseUnlock),
            DELIVERY_ACK_TAG => Some(Self::DeliveryAcknowledgement),
            SERVICE_REQ_TAG => Some(Self::ServiceRequest),
            _ => None,
        }
    }

    #[inline(always)]
    pub fn size_hint(&self) -> usize {
        match self {
            FrameTag::HandshakeRequest => 57,
            FrameTag::HandshakeResponse => 43,
            FrameTag::HandshakeResponseUnlock => 179,
            FrameTag::DeliveryAcknowledgement => 97,
            FrameTag::ServiceRequest => 33,
            FrameTag::TerminationSignal => 1,
        }
    }
}

/// Frame variants for different requests and responses
///
/// All frames are prefixed with a [`FrameTag`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HandshakeFrame {
    /// Client request to initiate a handshake.
    ///
    /// Clients can optionally specify a previous lane to resume in the event of a disconnection.
    HandshakeRequest {
        version: u8,
        supported_compression_bitmap: u8,
        pubkey: BlsPublicKey,
        resume_lane: Option<u8>,
    },
    /// Node response to assign an open lane.
    HandshakeResponse {
        lane: u8,
        pubkey: Secp256k1PublicKey,
        nonce: Nonce,
    },
    /// Node response to confirm resuming a lane.
    HandshakeResponseUnlock {
        pubkey: Secp256k1PublicKey,
        nonce: Nonce,
        lane: u8,
        last_bytes: u64,
        last_service_id: [u8; 32],
        last_signature: BlsSignature,
    },
    /// Client acknowledgment that a block was delivered.
    /// These are batched and submitted by the node for rewards
    DeliveryAcknowledgement { signature: BlsSignature },
    /// Client request to start a service subprotocol
    ServiceRequest { service_id: Hash },
    /// Signal from the node the connection was terminated, with a reason.
    TerminationSignal(Reason),
}

impl HandshakeFrame {
    /// Return the frame's tag or `None` if the frame is a signal.
    #[inline(always)]
    pub fn tag(&self) -> FrameTag {
        match self {
            Self::HandshakeRequest { .. } => FrameTag::HandshakeRequest,
            Self::HandshakeResponse { .. } => FrameTag::HandshakeResponse,
            Self::HandshakeResponseUnlock { .. } => FrameTag::HandshakeResponseUnlock,
            Self::DeliveryAcknowledgement { .. } => FrameTag::DeliveryAcknowledgement,
            Self::ServiceRequest { .. } => FrameTag::ServiceRequest,
            Self::TerminationSignal(_) => FrameTag::TerminationSignal,
        }
    }

    /// Return an estimation of the number of bytes this frame will need.
    #[inline]
    pub fn size_hint(&self) -> usize {
        self.tag().size_hint()
    }
}

#[derive(Debug)]
pub enum HandshakeCodecError {
    InvalidNetwork,
    InvalidTag(u8),
    InvalidReason(u8),
    UnexpectedFrame(FrameTag),
    ZeroLengthBlock,
    Io(std::io::Error),
    OccupiedLane,
    Unknown,
}

impl From<std::io::Error> for HandshakeCodecError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<HandshakeCodecError> for std::io::Error {
    fn from(value: HandshakeCodecError) -> Self {
        match value {
            HandshakeCodecError::Io(e) => e,
            error => Error::new(ErrorKind::Other, format!("{error:?}")),
        }
    }
}

/// Implementation for reading and writing handshake frames on a connection.
pub struct HandshakeConnection<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> {
    pub reader: R,
    pub writer: W,
    buffer: BytesMut,
}

impl<R, W> HandshakeConnection<R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    #[inline(always)]
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            // The maximum frame size is 179, so it should be enough to read into at all times
            buffer: BytesMut::with_capacity(179),
        }
    }

    #[inline(always)]
    pub async fn write_frame(&mut self, frame: HandshakeFrame) -> std::io::Result<()> {
        match frame {
            HandshakeFrame::TerminationSignal(reason) => {
                self.writer.write_u8(reason as u8).await?;
            },
            HandshakeFrame::HandshakeRequest {
                version,
                pubkey,
                supported_compression_bitmap,
                resume_lane: lane,
            } => {
                let mut buf = ArrayVec::<u8, 57>::new_const();
                debug_assert_eq!(NETWORK.len(), 5);

                buf.push(FrameTag::HandshakeRequest as u8);
                buf.write_all(&NETWORK).unwrap();
                buf.push(version);
                buf.push(supported_compression_bitmap);
                buf.push(lane.unwrap_or(0xFF));
                buf.write_all(&pubkey).unwrap();

                self.writer.write_all(&buf).await?;
            },
            HandshakeFrame::HandshakeResponse {
                pubkey,
                nonce,
                lane,
            } => {
                let mut buf = ArrayVec::<u8, 43>::new_const();

                buf.push(FrameTag::HandshakeResponse as u8);
                buf.push(lane);
                buf.write_all(&pubkey).unwrap();
                buf.write_all(&nonce.to_be_bytes()).unwrap();

                self.writer.write_all(&buf).await?;
            },
            HandshakeFrame::HandshakeResponseUnlock {
                pubkey,
                nonce,
                lane,
                last_service_id,
                last_bytes,
                last_signature,
            } => {
                let mut buf = ArrayVec::<u8, 179>::new_const();

                buf.push(FrameTag::HandshakeResponseUnlock as u8);
                buf.push(lane);
                buf.write_all(&pubkey).unwrap();
                buf.write_all(&nonce.to_be_bytes()).unwrap();
                buf.write_all(&last_service_id).unwrap();
                buf.write_all(&last_bytes.to_be_bytes()).unwrap();
                buf.write_all(&last_signature).unwrap();

                self.writer.write_all(&buf).await?;
            },
            HandshakeFrame::DeliveryAcknowledgement { signature } => {
                let mut buf = ArrayVec::<u8, 97>::new_const();

                buf.push(FrameTag::DeliveryAcknowledgement as u8);
                buf.write_all(&signature).unwrap();

                self.writer.write_all(&buf).await?;
            },
            HandshakeFrame::ServiceRequest { service_id } => {
                let mut buf = ArrayVec::<u8, 33>::new_const();

                buf.push(FrameTag::ServiceRequest as u8);
                buf.write_all(&service_id).unwrap();

                self.writer.write_all(&buf).await?;
            },
        }

        Ok(())
    }

    #[inline(always)]
    pub async fn read_frame(
        &mut self,
        filter: Option<u8>,
    ) -> std::io::Result<Option<HandshakeFrame>> {
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
    fn parse_frame(&mut self, filter: Option<u8>) -> std::io::Result<Option<HandshakeFrame>> {
        let len = self.buffer.len();
        if len == 0 {
            return Ok(None);
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
            FrameTag::HandshakeRequest => {
                let buf = self.buffer.split_to(size_hint);
                let network = &buf[1..6];
                if network != NETWORK {
                    return Err(HandshakeCodecError::InvalidNetwork.into());
                }

                let version = buf[6];
                let supported_compression_bitmap = buf[7];
                let lane = match buf[8] {
                    0xFF => None,
                    v => Some(v),
                };
                let pubkey = *array_ref!(buf, 9, 48);

                Ok(Some(HandshakeFrame::HandshakeRequest {
                    version,
                    supported_compression_bitmap,
                    resume_lane: lane,
                    pubkey,
                }))
            },
            FrameTag::HandshakeResponse => {
                let buf = self.buffer.split_to(size_hint);
                let lane = buf[1];
                let pubkey = *array_ref!(buf, 2, 33);
                let nonce = u64::from_be_bytes(*array_ref!(buf, 35, 8));

                Ok(Some(HandshakeFrame::HandshakeResponse {
                    pubkey,
                    nonce,
                    lane,
                }))
            },
            FrameTag::HandshakeResponseUnlock => {
                let buf = self.buffer.split_to(size_hint);
                let lane = buf[1];
                let pubkey = *array_ref!(buf, 2, 33);
                let nonce = u64::from_be_bytes(*array_ref!(buf, 35, 8));
                let last_service_id = *array_ref!(buf, 43, 32);
                let last_bytes = u64::from_be_bytes(*array_ref!(buf, 75, 8));
                let last_signature = *array_ref!(buf, 83, 96);

                Ok(Some(HandshakeFrame::HandshakeResponseUnlock {
                    pubkey,
                    nonce,
                    lane,
                    last_service_id,
                    last_bytes,
                    last_signature,
                }))
            },
            FrameTag::DeliveryAcknowledgement => {
                let buf = self.buffer.split_to(size_hint);
                let signature = *array_ref!(buf, 1, 96);

                Ok(Some(HandshakeFrame::DeliveryAcknowledgement { signature }))
            },
            FrameTag::ServiceRequest => {
                let buf = self.buffer.split_to(size_hint);
                let service_id = *array_ref!(buf, 1, 32);

                Ok(Some(HandshakeFrame::ServiceRequest { service_id }))
            },
            FrameTag::TerminationSignal => {
                let buf = self.buffer.split_to(size_hint);

                if let Some(reason) = Reason::from_u8(buf[0]) {
                    Ok(Some(HandshakeFrame::TerminationSignal(reason)))
                } else {
                    Err(HandshakeCodecError::InvalidReason(buf[0]).into())
                }
            },
        }
    }

    /// Write a termination signal to the stream.
    #[inline(always)]
    pub async fn termination_signal(&mut self, reason: Reason) -> std::io::Result<()> {
        self.write_frame(HandshakeFrame::TerminationSignal(reason))
            .await
    }

    /// Finish the connection, consuming the struct and returning the reader and writer.
    pub fn finish(self) -> (R, W) {
        (self.reader, self.writer)
    }
}

#[cfg(test)]
mod tests {
    use tokio::{
        net::{TcpListener, TcpStream},
        sync::mpsc::channel,
    };

    use super::*;

    type TResult = Result<(), HandshakeCodecError>;

    async fn encode_decode(frame: HandshakeFrame) -> TResult {
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

        // create a raw ufdp connection to encode/decode with
        let (r, w) = alice_stream.split();
        let mut alice = HandshakeConnection::new(r, w);
        let (r, w) = bob_stream.split();
        let mut bob = HandshakeConnection::new(r, w);

        // write/read the frame, comparing the result afterwards
        alice.write_frame(frame.clone()).await?;
        let recv_frame = bob.read_frame(None).await?.unwrap();
        assert_eq!(frame, recv_frame);

        Ok(())
    }

    #[tokio::test]
    async fn handshake_req() -> TResult {
        encode_decode(HandshakeFrame::HandshakeRequest {
            version: 0,
            supported_compression_bitmap: 0,
            resume_lane: None,
            pubkey: [1u8; 48],
        })
        .await
    }

    #[tokio::test]
    async fn handshake_res() -> TResult {
        encode_decode(HandshakeFrame::HandshakeResponse {
            lane: 0,
            nonce: 1000,
            pubkey: [1; 33],
        })
        .await?;

        encode_decode(HandshakeFrame::HandshakeResponseUnlock {
            lane: 0,
            nonce: 1000,
            pubkey: [2; 33],
            last_service_id: [0; 32],
            last_bytes: 1000,
            last_signature: [3; 96],
        })
        .await
    }

    #[tokio::test]
    async fn service_req() -> TResult {
        encode_decode(HandshakeFrame::ServiceRequest {
            service_id: [0; 32],
        })
        .await
    }

    #[tokio::test]
    async fn decryption_key_req() -> TResult {
        encode_decode(HandshakeFrame::DeliveryAcknowledgement { signature: [1; 96] }).await
    }

    #[tokio::test]
    async fn termination_signal() -> TResult {
        encode_decode(HandshakeFrame::TerminationSignal(Reason::Unknown)).await
    }
}
