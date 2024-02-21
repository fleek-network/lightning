mod handler;
mod opts;
mod runtime;
mod tensor;

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
///
/// Note: for each variant, the same encoding will be used for the output.
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Input {
    /// The input is an array.
    #[serde(rename = "array")]
    Array { encoding: Encoding, data: Bytes },
    /// The input is a hash map.
    ///
    /// Keys will be passed directly to the session
    /// along with their corresponding value.
    #[serde(rename = "map")]
    Map {
        encoding: Encoding,
        data: HashMap<String, Bytes>,
    },
}

/// Input encoding.
#[derive(Serialize, Deserialize)]
pub enum Encoding {
    /// Input is an 8-bit unsigned-int array.
    ///
    /// This input will be interpreted as an array of dimension 1
    /// before feeding it to the model.
    #[serde(rename = "raw")]
    Raw,
    /// Input is a npy file.
    ///
    /// This input will be converted into an `ndarray`.
    #[serde(rename = "npy")]
    Npy,
}

/// Output from a session run.
#[derive(Deserialize, Serialize)]
pub struct Output {
    pub encoding: Encoding,
    pub outputs: HashMap<String, String>,
}

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
