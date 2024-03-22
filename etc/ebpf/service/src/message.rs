use std::io;
use std::net::{Ipv4Addr, SocketAddrV4};

use bytes::{Buf, BufMut, Bytes, BytesMut};

pub struct Pf {
    pub op: u8,
    pub addr: SocketAddrV4,
}

impl Pf {
    /// Add to block list.
    pub const ADD: u8 = 0;
    /// Remove from block list.
    pub const REMOVE: u8 = 1;
}

impl From<Pf> for Bytes {
    fn from(value: Pf) -> Self {
        let mut result = BytesMut::new();
        result.put_u8(value.op);
        result.put_slice(&value.addr.ip().octets());
        result.put_u16(value.addr.port());
        result.freeze()
    }
}

impl TryFrom<Bytes> for Pf {
    type Error = io::Error;

    fn try_from(mut value: Bytes) -> Result<Self, Self::Error> {
        if value.len() != 7 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid"));
        }
        let op = value.get_u8();
        let ip = Ipv4Addr::from(value.get_u32());
        let port = value.get_u16();
        let addr = SocketAddrV4::new(ip, port);
        Ok(Pf { op, addr })
    }
}
