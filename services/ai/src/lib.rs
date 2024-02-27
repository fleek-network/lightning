mod array;
mod handler;
mod opts;
mod runtime;

use std::collections::HashMap;

use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::opts::{Device, Origin};

/// Header for stream-based protocols.
///
/// This header must be sent before sending an [`Input`].
/// Note: do not send this header when using HTTP.
#[derive(Deserialize, Serialize)]
pub struct StartSession {
    pub model: String,
    pub origin: Origin,
    pub device: Device,
}

/// Input for inference.
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

/// Output from an inference run.
pub type Output = HashMap<String, EncodedArrayExt>;

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename = "_ExtStruct")]
// See https://docs.rs/rmp-serde/latest/rmp_serde/constant.MSGPACK_EXT_STRUCT_NAME.html.
pub struct EncodedArrayExt((i8, Bytes));

#[tokio::main]
pub async fn main() {
    fn_sdk::ipc::init_from_env();
    tracing::info!("Initialized AI service!");

    // Entry point of the Onnx runtime.
    if let Err(e) = ort::init().with_name("fleek-ai-inference").commit() {
        tracing::error!("failed to initialize the Onnx runtime: {e}");
    }

    let mut listener = fn_sdk::ipc::conn_bind().await;
    while let Ok(conn) = listener.accept().await {
        tokio::spawn(async move {
            if let Err(e) = handler::handle(conn).await {
                tracing::info!("there was an error when handling the connection: {e:?}");
            }
        });
    }
}
