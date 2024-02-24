use std::collections::HashMap;

use bytes::Bytes;
// Todo: these should be imported from service but
// there is currently a linker-related issue
// that is preventing us.
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
#[repr(u8)]
pub enum Origin {
    Blake3,
    Ipfs,
    Http,
    Unknown,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Device {
    Cpu,
    // This is not supported atm.
    Cuda(usize),
}

#[derive(Deserialize, Serialize)]
pub struct StartSession {
    pub model: String,
    pub origin: Origin,
    pub device: Device,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename = "_ExtStruct")]
pub struct EncodedArrayExt(pub (i8, Bytes));

#[derive(Deserialize, Serialize, Debug)]
pub enum Input {
    /// The input is an array.
    #[serde(rename = "array")]
    Array { data: EncodedArrayExt },
    /// The input is a hash map.
    ///
    /// Keys will be passed directly to the session
    /// along with their corresponding value.
    #[serde(rename = "map")]
    Map {
        data: HashMap<String, EncodedArrayExt>,
    },
}
