//! The network messages for Fleek Network. The point of this crate is to have one common
//! place where we define the schema for the messages that are part of the network stack.
//!
//! Although serde is great, we want to make it easy to use a schema-based encoder/decoder
//! in the future (such as flatbuffers or protobuf).
//!
//! However, that task can be extra development effort at the current stage, so we would
//! rather use serde for the time being. This crate serves as a layer of abstraction
//! that works over serde + flexbuffers (schema-less cousin of flatbuffers), and makes
//! the entire encoding/decoding effort hot-swappable in one place.
//!
//! Anywhere we need to care about serialization and deserialization of messages we can
//! rely on the [`LightningMessage`] trait to use the encoding and decoding.

use std::io::Write;

use serde::{Deserialize, Serialize};

pub mod broadcast;
pub mod envelope;
pub mod handshake;
pub mod task_broker;

/// Any networking message that can be serialized and deserialized.
pub trait LightningMessage: Sized + Send + Sync + 'static {
    /// Decode the message. Returns an error if we can't deserialize.
    fn decode(buffer: &[u8]) -> anyhow::Result<Self>;

    /// Encode the message. Return the result from the `writer.write`
    fn encode<W: Write>(&self, writer: &mut W) -> std::io::Result<()>;

    /// Encode the message with length delimited. Return the result from the `writer.write`
    /// The length is written as a u64 in little endian.
    /// This is useful for framing the message in a stream.
    fn encode_length_delimited<W: Write>(&self, writer: &mut W) -> std::io::Result<()>;
}

/// A marker type which enables auto implementation for serde serialize and deserialize. This
/// allows us to skip the auto-trait implementation in case we prefer a custom serialization
/// for a single type. But any other type should have the `impl AutoImplSerde for T`.
pub trait AutoImplSerde: Serialize + for<'a> Deserialize<'a> + Send + Sync + Sized {}

// Auto implement the lighting message for any serde type using flex buffer.
// This is to allow us to move forward without a fixed schema for the time being and to
// have a faster development. However an schema based approach should be preferred in
// the future.

impl<T: AutoImplSerde + 'static> LightningMessage for T {
    #[inline]
    fn decode(buffer: &[u8]) -> anyhow::Result<Self> {
        let der = flexbuffers::Reader::get_root(buffer)?;
        let tmp = Self::deserialize(der)?;
        Ok(tmp)
    }

    #[inline]
    fn encode<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let mut ser = flexbuffers::FlexbufferSerializer::new();
        self.serialize(&mut ser).expect("Failed to serialize");
        writer.write_all(ser.view())
    }

    fn encode_length_delimited<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let mut ser = flexbuffers::FlexbufferSerializer::new();
        self.serialize(&mut ser).expect("Failed to serialize");

        let len = ser.view().len() as u32;
        writer.write_all(&len.to_le_bytes())?;
        writer.write_all(ser.view())
    }
}

impl AutoImplSerde for () {}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Serialize, Deserialize, Eq, PartialEq, Debug)]
    struct Person {
        name: String,
        age: u8,
    }

    impl AutoImplSerde for Person {}

    #[test]
    fn encdec_should_work() {
        let person = Person {
            name: "Fleek".into(),
            age: 54,
        };
        let mut buffer = Vec::new();
        person.encode(&mut buffer).expect("Failed to write.");
        let decoded = Person::decode(buffer.as_slice()).unwrap();
        assert_eq!(decoded, person);
    }
}
