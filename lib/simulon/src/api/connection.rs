use serde::{de::DeserializeOwned, Serialize};

use super::RemoteAddr;
use crate::state::{with_node, ResourceId};

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

    pub fn is_closed(&self) -> bool {
        self.reader.is_closed()
    }

    /// Send a message through the connection.
    pub fn write<T>(&mut self, message: &T)
    where
        T: Serialize,
    {
        self.writer.write(message)
    }

    pub async fn recv<T>(&mut self) -> Option<T>
    where
        T: DeserializeOwned,
    {
        self.reader.recv::<T>().await
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
    pub fn is_closed(&self) -> bool {
        with_node(|n| !n.is_connection_open(self.rid))
    }

    /// Receive a message from the connection.
    pub async fn recv<T>(&mut self) -> Option<T>
    where
        T: DeserializeOwned,
    {
        if self.is_closed() {
            return None;
        }

        let bytes = with_node(|n| n.recv(self.rid)).await?;
        Some(bincode::deserialize(&bytes).unwrap())
    }
}

impl Writer {
    /// Send a message through the connection.
    pub fn write<T>(&mut self, message: &T)
    where
        T: Serialize,
    {
        let bytes = bincode::serialize(message).expect("Serialization failed.");
        with_node(|n| n.send(self.remote, self.remote_rid, bytes))
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        with_node(|n| {
            n.close_connection(self.reader.rid, self.writer.remote, self.writer.remote_rid)
        })
    }
}
