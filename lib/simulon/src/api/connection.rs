use super::RemoteAddr;
use crate::state::ResourceId;

pub struct Connection {
    reader: Reader,
    writer: Writer,
}

impl Connection {
    pub(super) fn new(rid: ResourceId, remote: RemoteAddr, remote_rid: ResourceId) -> Self {
        Self {
            reader: Reader { rid },
            writer: Writer { remote, remote_rid },
        }
    }

    /// Split the connection to the reader and writer half.
    pub fn split(&mut self) -> (&mut Reader, &mut Writer) {
        (&mut self.reader, &mut self.writer)
    }

    /// Returns the remote of this connection.
    pub fn remote(&self) -> RemoteAddr {
        self.writer.remote
    }
}

pub struct Reader {
    rid: ResourceId,
}

pub struct Writer {
    remote: RemoteAddr,
    remote_rid: ResourceId,
}

impl Reader {
    async fn recv<T>(&mut self) -> Option<T> {
        todo!()
    }
}

impl Writer {
    fn write<T>(&mut self, _message: T) {
        todo!()
    }
}

impl Drop for Connection {
    fn drop(&mut self) {}
}
