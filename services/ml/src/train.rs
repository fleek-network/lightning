use serde::{Deserialize, Serialize};

/// Options for training.
#[derive(Debug, Deserialize, Serialize)]
pub struct Opts {
    /// Number of epochs.
    epochs: u32,
    /// Uri for getting the models from origin.
    model_uri: u32,
    /// Uri for getting the training set from origin.
    train_set_uri: String,
    /// Uri for getting the validation set from origin.
    validation_set_uri: String,
}
