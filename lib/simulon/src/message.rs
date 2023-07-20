use crate::{api::RemoteAddr, state::ResourceId};

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Message {
    pub time: u128,
    pub sender: RemoteAddr,
    pub receiver: RemoteAddr,
    pub detail: MessageDetail,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum MessageDetail {
    Connect {
        port: u16,
        rid: ResourceId,
    },
    ConnectionAccepted {
        source_rid: ResourceId,
        remote_rid: ResourceId,
    },
    ConnectionRefused {
        source: RemoteAddr,
        source_rid: ResourceId,
    },
    ConnectionClosed {
        rid: ResourceId,
    },
    Data {
        data: Vec<u8>,
        rid: ResourceId,
    },
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::state::ResourceId;

    #[test]
    fn test_ordering() {
        let m1 = Message {
            time: 0,
            sender: RemoteAddr(1),
            receiver: RemoteAddr(2),
            detail: super::MessageDetail::ConnectionClosed {
                rid: ResourceId(18),
            },
        };

        let m2 = Message {
            time: 17,
            sender: RemoteAddr(1),
            receiver: RemoteAddr(2),
            detail: super::MessageDetail::ConnectionClosed {
                rid: ResourceId(18),
            },
        };

        let m3 = Message {
            time: 3,
            sender: RemoteAddr(0),
            receiver: RemoteAddr(0),
            detail: super::MessageDetail::ConnectionClosed { rid: ResourceId(0) },
        };

        let m4 = Message {
            time: 5,
            sender: RemoteAddr(0),
            receiver: RemoteAddr(0),
            detail: super::MessageDetail::ConnectionClosed { rid: ResourceId(0) },
        };

        let m5 = Message {
            time: 2,
            sender: RemoteAddr(18),
            receiver: RemoteAddr(0),
            detail: super::MessageDetail::ConnectionClosed { rid: ResourceId(0) },
        };

        let mut set: BTreeSet<Message> = BTreeSet::new();

        set.insert(m1);
        set.insert(m2);
        set.insert(m3);
        set.insert(m4);
        set.insert(m5);

        assert_eq!(set.pop_first().unwrap().time, 0);
        assert_eq!(set.pop_first().unwrap().time, 2);
        assert_eq!(set.pop_first().unwrap().time, 3);
        assert_eq!(set.pop_first().unwrap().time, 5);
        assert_eq!(set.pop_first().unwrap().time, 17);
    }
}
