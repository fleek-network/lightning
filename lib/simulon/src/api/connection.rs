use super::RemoteAddr;
use crate::state::ResourceId;

pub struct Connection {
    rid: ResourceId,
    remote: RemoteAddr,
    remote_rid: ResourceId,
    reader: Reader,
    writer: Writer,
}

impl Connection {
    pub(super) fn new(rid: ResourceId, remote: RemoteAddr, remote_rid: ResourceId) -> Self {
        Self {
            rid,
            remote,
            remote_rid,
            reader: Reader {},
            writer: Writer {},
        }
    }

    /// Split the connection to the reader and writer half.
    pub fn split(&mut self) -> (&mut Reader, &mut Writer) {
        (&mut self.reader, &mut self.writer)
    }

    /// Returns the remote of this connection.
    pub fn remote(&self) -> RemoteAddr {
        self.remote
    }
}

pub struct Reader {}

pub struct Writer {}

impl Reader {
    async fn recv<T>(&mut self) -> Option<T> {
        todo!()
    }
}

impl Drop for Connection {
    fn drop(&mut self) {}
}
