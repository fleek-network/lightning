mod handler;
mod libtorch;
mod opts;

use bytes::Bytes;
pub use opts::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Infer(Infer),
    Train(Train),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Infer {
    /// Input for inference.
    ///
    /// This is a serialized tensor.
    /// Please see https://pytorch.org/docs/stable/notes/serialization.html.
    pub input: Bytes,
    /// Uri of the model.
    pub model: String,
    /// Origin to use for getting the model.
    pub origin: Origin,
    /// Uri of pre-trained weights.
    pub weights: Option<String>,
    /// Devices on which tensor computations are run.
    pub device: Device,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Train {
    /// Devices on which tensor computations are run.
    pub device: Device,
    /// Origin to use for all fetches.
    pub origin: Origin,
    /// Uri for getting the models from origin.
    pub model_uri: String,
    /// Uri for getting the training data from origin.
    pub train_data_uri: String,
    /// Uri for getting the training labels from origin.
    pub train_label_uri: String,
    /// Uri for getting the validation data from origin.
    pub validation_data_uri: String,
    /// Uri for getting the validation labels from origin.
    pub validation_label_uri: String,
}

#[tokio::main]
pub async fn main() {
    fn_sdk::ipc::init_from_env();
    tracing::info!("Initialized AI service!");

    let mut listener = fn_sdk::ipc::conn_bind().await;
    while let Ok(conn) = listener.accept().await {
        tokio::spawn(async move {
            if let Err(e) = handler::handle(conn).await {
                tracing::info!("there was an error when handling the connection: {e:?}");
            }
        });
    }
}
