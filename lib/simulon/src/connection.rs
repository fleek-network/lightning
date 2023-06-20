use crate::{engine::Rid, peer::PeerId};

/// A connection from/to the current node.
pub struct Connection {
    /// The unique resource id used by this connection.
    rid: Rid,
    /// The other peer that this connection is to.
    peer: PeerId,
    /// Is the current node the peer that initiated the connection? Or in other words is
    /// this connection object initiated from a call to `dial`.
    is_initiator: bool,
}

impl Connection {
    pub fn split(&mut self) -> (&mut Reader, &mut Writer) {
        todo!()
    }

    pub fn is_open(&self) -> bool {
        todo!()
    }

    pub fn close(self) {
        todo!()
    }
}

pub struct Reader {}

impl Reader {
    pub async fn recv(&mut self) {
        todo!()
    }
}

pub struct Writer {}

impl Writer {
    pub fn write(&mut self, _message: ()) {
        todo!()
    }
}
