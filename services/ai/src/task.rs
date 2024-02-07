use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::Device;

#[derive(Debug, Serialize, Deserialize)]
pub struct Run {
    /// Input for this run.
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

#[derive(Debug, Serialize, Deserialize)]
#[repr(u8)]
pub enum Origin {
    Blake3,
    Ipfs,
    Http,
    Unknown,
}

impl From<&str> for Origin {
    fn from(s: &str) -> Self {
        match s {
            "blake3" => Self::Blake3,
            "ipfs" => Self::Ipfs,
            "http" => Self::Http,
            _ => Self::Unknown,
        }
    }
}
