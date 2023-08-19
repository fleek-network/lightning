use std::rc::Rc;

use serde::de::DeserializeOwned;
use serde::Serialize;

use super::RemoteAddr;
use crate::state::{with_node, ResourceId};

/// A connection to a remote peer. You can establish a connection by either connecting to a peer
/// via the [`crate::api::connect`] method or by accepting connections from a listener.
pub struct Connection {
    rid: ResourceId,
    remote: RemoteAddr,
    remote_rid: ResourceId,
}

impl Connection {
    pub(super) fn new(rid: ResourceId, remote: RemoteAddr, remote_rid: ResourceId) -> Self {
        Self {
            rid,
            remote,
            remote_rid,
        }
    }

    /// Split connection to owned instances of a reader and a writer.
    pub fn split(self) -> (OwnedReader, OwnedWriter) {
        let connection = Rc::new(self);
        (
            OwnedReader {
                connection: connection.clone(),
            },
            OwnedWriter { connection },
        )
    }

    /// Returns the address of the other end of this connection.
    pub fn remote(&self) -> RemoteAddr {
        self.remote
    }

    /// Returns true if the connection is closed.
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
        with_node(|n| n.close_connection(self.rid, self.remote, self.remote_rid))
    }
}

pub struct OwnedReader {
    connection: Rc<Connection>,
}

pub struct OwnedWriter {
    connection: Rc<Connection>,
}

impl OwnedReader {
    /// Returns the address of the other end of this connection.
    pub fn remote(&self) -> RemoteAddr {
        self.connection.remote
    }

    /// Returns true if this connection is closed.
    pub fn is_closed(&self) -> bool {
        self.connection.is_closed()
    }

    /// Receive data from the connection.
    pub async fn recv<T>(&mut self) -> Option<T>
    where
        T: DeserializeOwned,
    {
        if self.is_closed() {
            return None;
        }

        let bytes = with_node(|n| n.recv(self.connection.rid)).await?;
        Some(bincode::deserialize(&bytes).unwrap())
    }
}

impl OwnedWriter {
    /// Returns the address of the other end of this connection.
    pub fn remote(&self) -> RemoteAddr {
        self.connection.remote
    }

    /// Returns true if this connection is closed.
    pub fn is_closed(&self) -> bool {
        self.connection.is_closed()
    }

    /// Send a message through the connection.
    pub fn write<T>(&mut self, message: &T)
    where
        T: Serialize,
    {
        let bytes = bincode::serialize(message).expect("Serialization failed.");
        with_node(|n| n.send(self.connection.remote, self.connection.remote_rid, bytes))
    }
}
