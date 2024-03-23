use std::io;
use std::net::{Ipv4Addr, SocketAddrV4};

use bytes::{Buf, BufMut, Bytes, BytesMut};

pub enum EbpfServiceFrame {
    /// Mandatory access control.
    Mac,
    /// Packet filter.
    Pf(Pf),
}

impl EbpfServiceFrame {
    const PF: u8 = 0;
    const MAC: u8 = 1;
}

impl TryFrom<Bytes> for EbpfServiceFrame {
    type Error = io::Error;

    fn try_from(mut value: Bytes) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "not enough data to deserialize frame",
            ));
        }

        let frame = match value.get_u8() {
            EbpfServiceFrame::MAC => EbpfServiceFrame::Mac,
            EbpfServiceFrame::PF => {
                let pf = Pf::try_from(value)?;
                EbpfServiceFrame::Pf(pf)
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

impl From<EbpfServiceFrame> for Bytes {
    fn from(value: EbpfServiceFrame) -> Self {
        match value {
            EbpfServiceFrame::Mac => {
                let mut result = BytesMut::with_capacity(1);
                result.put_u8(EbpfServiceFrame::MAC);
                result.freeze()
            },
            EbpfServiceFrame::Pf(pf) => {
                let mut result = BytesMut::with_capacity(8);
                result.put_u8(EbpfServiceFrame::PF);
                pf.serialize(&mut result)
                    .expect("Buffer capacity is hard-coded");
                result.freeze()
            },
        }
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

impl TryFrom<Bytes> for Pf {
    type Error = io::Error;

    fn try_from(mut value: Bytes) -> Result<Self, Self::Error> {
        if value.len() != 7 {
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
