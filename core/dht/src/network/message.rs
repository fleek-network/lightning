use anyhow::Result;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use lightning_interfaces::types::NodeIndex;
use serde::{Deserialize, Serialize};

use crate::table::worker::TableKey;

pub const PING_TYPE: u8 = 0;
pub const PONG_TYPE: u8 = 1;
pub const STORE_TYPE: u8 = 2;
pub const FIND_VALUE_TYPE: u8 = 3;
pub const FIND_VALUE_RESPONSE_TYPE: u8 = 4;
pub const FIND_NODE_TYPE: u8 = 5;
pub const FIND_NODE_RESPONSE_TYPE: u8 = 6;

pub fn ping(id: u32, token: u32, from: NodeIndex) -> Message {
    Message::new(id, token, from, PING_TYPE, Bytes::new())
}

pub fn pong(id: u32, token: u32, from: NodeIndex) -> Message {
    Message::new(id, token, from, PONG_TYPE, Bytes::new())
}

pub fn store(id: u32, token: u32, from: NodeIndex, value: Bytes) -> Message {
    let bytes = Bytes::from(Store { value });
    Message::new(id, token, from, STORE_TYPE, bytes)
}

pub fn find_value(id: u32, token: u32, from: NodeIndex, key: TableKey) -> Message {
    let bytes = Bytes::from(Find { key });
    Message::new(id, token, from, FIND_VALUE_TYPE, bytes)
}

pub fn find_node(id: u32, token: u32, from: NodeIndex, key: TableKey) -> Message {
    let bytes = Bytes::from(Find { key });
    Message::new(id, token, from, FIND_NODE_TYPE, bytes)
}

pub fn find_value_response(
    id: u32,
    token: u32,
    from: NodeIndex,
    contacts: Vec<NodeIndex>,
    value: Bytes,
) -> Message {
    let bytes = Bytes::from(FindResponse { contacts, value });
    Message::new(id, token, from, FIND_VALUE_RESPONSE_TYPE, bytes)
}

pub fn find_node_response(
    id: u32,
    token: u32,
    from: NodeIndex,
    contacts: Vec<NodeIndex>,
) -> Message {
    let bytes = Bytes::from(FindResponse {
        contacts,
        value: Bytes::new(),
    });
    Message::new(id, token, from, FIND_NODE_RESPONSE_TYPE, bytes)
}

pub fn _find_response_in_parts(
    id: u32,
    token: u32,
    from: NodeIndex,
    contacts: Vec<NodeIndex>,
    max_size: usize,
) -> Vec<Message> {
    let mut buf = Vec::new();
    for chunk in contacts.chunks(max_size) {
        buf.push(find_node_response(id, token, from, chunk.to_vec()))
    }
    buf
}

pub struct Store {
    pub value: Bytes,
}

impl From<Store> for Bytes {
    fn from(value: Store) -> Self {
        value.value
    }
}

impl From<Bytes> for Store {
    fn from(value: Bytes) -> Self {
        Self { value }
    }
}

#[derive(Deserialize, Serialize)]
pub struct Find {
    key: TableKey,
}

impl Find {
    #[inline]
    pub fn key(&self) -> &TableKey {
        &self.key
    }
}

impl From<Find> for Bytes {
    fn from(value: Find) -> Self {
        // Todo: Remove bincode.
        bincode::serialize(&value).expect("Typed value").into()
    }
}

impl TryFrom<Bytes> for Find {
    type Error = anyhow::Error;

    fn try_from(value: Bytes) -> std::result::Result<Self, Self::Error> {
        // Todo: Remove bincode.
        bincode::deserialize(&value).map_err(Into::into)
    }
}

#[derive(Deserialize, Serialize)]
pub struct FindResponse {
    contacts: Vec<NodeIndex>,
    // This might not be used because an immediate requirement is that indexer
    // handles mapping and not actual application data.
    value: Bytes,
}

impl FindResponse {
    #[inline]
    pub fn contacts(&self) -> &[NodeIndex] {
        &self.contacts
    }

    #[inline]
    pub fn content(&self) -> Bytes {
        self.value.clone()
    }
}

impl From<FindResponse> for Bytes {
    fn from(value: FindResponse) -> Self {
        // Todo: Remove bincode.
        bincode::serialize(&value).expect("Typed value").into()
    }
}

impl TryFrom<Bytes> for FindResponse {
    type Error = anyhow::Error;

    fn try_from(value: Bytes) -> std::result::Result<Self, Self::Error> {
        // Todo: Remove bincode.
        bincode::deserialize(&value).map_err(Into::into)
    }
}

pub struct Message {
    // Todo: Maybe merge id and token to safe space.
    id: u32,
    token: u32,
    // Todo: find trust-less way of deriving the src node index.
    pub(crate) from: NodeIndex,
    ty: u8,
    bytes: Bytes,
}

impl Message {
    #[inline]
    fn new(id: u32, token: u32, from: NodeIndex, ty: u8, bytes: Bytes) -> Self {
        Self {
            id,
            token,
            from,
            ty,
            bytes,
        }
    }

    #[inline]
    pub fn id(&self) -> u32 {
        self.id
    }

    #[inline]
    pub fn token(&self) -> u32 {
        self.token
    }

    #[inline]
    pub fn ty(&self) -> u8 {
        self.ty
    }

    #[inline]
    pub fn bytes(self) -> Bytes {
        self.bytes
    }

    #[inline]
    pub fn decode(bytes: Bytes) -> Result<Self> {
        Self::try_from(bytes)
    }

    #[inline]
    pub fn encode(self) -> Bytes {
        Bytes::from(self)
    }
}

impl From<Message> for Bytes {
    fn from(value: Message) -> Self {
        let mut bytes = BytesMut::with_capacity(13 + value.bytes.len());
        bytes.put_u32(value.id);
        bytes.put_u32(value.token);
        bytes.put_u32(value.from);
        bytes.put_u8(value.ty);
        bytes.put(value.bytes);

        bytes.freeze()
    }
}

impl TryFrom<Bytes> for Message {
    type Error = anyhow::Error;

    fn try_from(mut value: Bytes) -> std::result::Result<Self, Self::Error> {
        if value.len() < 13 {
            anyhow::bail!("missing data")
        }

        let id = value.get_u32();
        let token = value.get_u32();
        let from = value.get_u32();
        let ty = value.get_u8();

        Ok(Self {
            id,
            token,
            from,
            ty,
            bytes: value,
        })
    }
}

// Todo: Add unit tests.
