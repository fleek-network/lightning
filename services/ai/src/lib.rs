mod handler;
mod opts;
mod runtime;
mod tensor;

use serde::{Deserialize, Serialize};

use crate::opts::{Device, Origin};

/// Header for stream-based protocols.
#[derive(Deserialize, Serialize)]
pub struct StartSession {
    pub model: String,
    pub origin: Origin,
    pub device: Device,
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
