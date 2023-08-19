use serde::de::DeserializeOwned;
use serde::Serialize;

/// The serialization backend that can empower [`Atomo`] with Serde for
/// serializing the data for the persistence layer.
pub trait SerdeBackend: 'static {
    fn serialize<T>(value: &T) -> Vec<u8>
    where
        T: Serialize;

    fn deserialize<T>(slice: &[u8]) -> T
    where
        T: DeserializeOwned;
}

/// The bincode serializer from the [`bincode`] crate.
pub struct BincodeSerde;

impl SerdeBackend for BincodeSerde {
    fn serialize<T>(value: &T) -> Vec<u8>
    where
        T: Serialize,
    {
        bincode::serialize(value).unwrap()
    }

    fn deserialize<T>(slice: &[u8]) -> T
    where
        T: DeserializeOwned,
    {
        bincode::deserialize(slice).unwrap()
    }
}
