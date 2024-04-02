use std::io;
use std::net::{Ipv4Addr, SocketAddrV4};

use bytes::{Buf, BufMut, Bytes, BytesMut};

pub enum EbpfServiceFrame {
    /// Mandatory access control.
    FileOpen(FileOpen),
    /// Packet filter.
    Pf(Pf),
}

impl EbpfServiceFrame {
    const PF: u8 = 0;
    const MAC: u8 = 1;

    // Serialize the frame prefixed with the length of the frame.
    pub fn serialize_len_delimit(self) -> Bytes {
        match self {
            EbpfServiceFrame::FileOpen(file_open) => {
                let size = file_open.size();
                let mut result = BytesMut::with_capacity(8 + file_open.size());
                result.put_u64(size as u64);
                file_open
                    .serialize(&mut result)
                    .expect("Buffer capacity is hard-coded");
                result.freeze()
            },
            EbpfServiceFrame::Pf(pf) => {
                let mut result = BytesMut::with_capacity(8 + 8);
                result.put_u64(8);
                result.put_u8(EbpfServiceFrame::PF);
                pf.serialize(&mut result)
                    .expect("Buffer capacity is hard-coded");
                result.freeze()
            },
        }
    }
}

impl TryFrom<&[u8]> for EbpfServiceFrame {
    type Error = io::Error;

    fn try_from(mut value: &[u8]) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "not enough data to deserialize frame",
            ));
        }

        let frame = match value.get_u8() {
            EbpfServiceFrame::MAC => {
                let file_open = FileOpen::try_from(value)?;
                EbpfServiceFrame::FileOpen(file_open)
            },
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

pub struct FileOpen {
    op: u8,
    src: FileOpenSrc,
}

pub enum FileOpenSrc {
    Pid(u64),
    BinPath(String),
}

impl FileOpen {
    /// Allow open on file.
    pub const ALLOW: u8 = 0;
    /// Block open on file.
    pub const BLOCK: u8 = 1;

    pub fn size(&self) -> usize {
        // One for the op and the other for the enum.
        let mut size = 2;
        match &self.src {
            FileOpenSrc::Pid(_) => {
                size += 8;
            },
            FileOpenSrc::BinPath(path) => size += path.len(),
        }
        size
    }

    pub fn serialize<T: BufMut>(self, mut buf: T) -> io::Result<()> {
        if buf.remaining_mut() < self.size() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not enough space in buffer",
            ));
        }

        buf.put_u8(self.op);

        match self.src {
            FileOpenSrc::Pid(pid) => {
                buf.put_u8(0);
                buf.put_u64(pid);
            },
            FileOpenSrc::BinPath(path) => {
                buf.put_u8(1);
                buf.put_slice(path.as_bytes());
            },
        }
        Ok(())
    }
}

impl TryFrom<&[u8]> for FileOpen {
    type Error = io::Error;

    fn try_from(mut value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() < 2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not enough data to deserialize",
            ));
        }
        let op = value.get_u8();
        let var = value.get_u8();

        if !value.has_remaining() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not enough data to deserialize",
            ));
        }

        let src = match var {
            0 if value.remaining() < 8 => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "not enough data to read pid",
                ));
            },
            0 => {
                let pid = value.get_u64();
                FileOpenSrc::Pid(pid)
            },
            1 if !value.has_remaining() => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "not enough data to read path",
                ));
            },
            1 => {
                // Todo: can we avoid the allocation here?
                let path = String::from_utf8(value.chunk().to_vec()).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "invalid path string")
                })?;
                FileOpenSrc::BinPath(path)
            },
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "unknwon variant",
                ));
            },
        };

        Ok(FileOpen { op, src })
    }
}
