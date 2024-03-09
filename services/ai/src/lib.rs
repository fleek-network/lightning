mod handler;
mod opts;
mod runtime;

use serde::{Deserialize, Serialize};

use crate::opts::{Device, Encoding, Format, Origin};

/// Header for stream-based protocols.
///
/// All of these parameters apply to the entire duration of the session.
/// This header must be sent before sending any input.
/// Note: do not send this header when using HTTP.
#[derive(Deserialize, Serialize)]
pub struct StartSession {
    /// CID of Onnx model.
    pub model: String,
    /// Origin to fetch model from.
    pub origin: Origin,
    /// Device to use for inference.
    pub device: Device,
    /// Format to use for requests and responses.
    pub content_format: Format,
    /// Encoding to use for the input and output arrays.
    pub model_io_encoding: Encoding,
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
