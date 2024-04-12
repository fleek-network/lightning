use std::io;
use std::net::{Ipv4Addr, SocketAddrV4};

use bytes::{Buf, BufMut, Bytes, BytesMut};

pub const PACKET_FILTER_SERVICE: u32 = 0;
pub const SUB_SECURITY_TOPIC: u32 = 1;
pub const SUB_STATS_TOPIC: u32 = 2;

pub enum Message {
    SecurityEvent,
    Stats,
}

#[non_exhaustive]
pub enum IpcServiceFrame {
    /// Packet filter.
    Pf(Pf),
}

impl IpcServiceFrame {
    const PF: u8 = 0;

    // Serialize the frame prefixed with the length of the frame.
    pub fn serialize_len_delimit(self) -> Bytes {
        match self {
            IpcServiceFrame::Pf(pf) => {
                let mut result = BytesMut::with_capacity(8 + 8);
                result.put_u64(8);
                result.put_u8(IpcServiceFrame::PF);
                pf.serialize(&mut result)
                    .expect("Buffer capacity is hard-coded");
                result.freeze()
            },
        }
    }
}

impl TryFrom<&[u8]> for IpcServiceFrame {
    type Error = io::Error;

    fn try_from(mut value: &[u8]) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "not enough data to deserialize frame",
            ));
        }

        let frame = match value.get_u8() {
            IpcServiceFrame::PF => {
                let pf = Pf::try_from(value)?;
                IpcServiceFrame::Pf(pf)
            },
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "unknown service type",
                ));
            },
        };

        Ok(frame)
    }
}

pub struct Pf {
    pub op: u8,
    pub addr: SocketAddrV4,
}

impl Pf {
    /// Add to block list.
    pub const ADD: u8 = 0;
    /// Remove from block list.
    pub const REMOVE: u8 = 1;

    pub fn serialize<T: BufMut>(self, mut buf: T) -> io::Result<()> {
        if buf.remaining_mut() < 7 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not enough space in buffer",
            ));
        }
        buf.put_u8(self.op);
        buf.put_slice(&self.addr.ip().octets());
        buf.put_u16(self.addr.port());
        Ok(())
    }
}

impl TryFrom<&[u8]> for Pf {
    type Error = io::Error;

    fn try_from(mut value: &[u8]) -> Result<Self, Self::Error> {
        if value.remaining() != 7 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not enough data to deserialize",
            ));
        }
        let op = value.get_u8();
        let ip = Ipv4Addr::from(value.get_u32());
        let port = value.get_u16();
        let addr = SocketAddrV4::new(ip, port);
        Ok(Pf { op, addr })
    }
}
