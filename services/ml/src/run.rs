use bytes::Bytes;
use serde::{Deserialize, Serialize};

/// Options for running a models.
#[derive(Debug, Deserialize, Serialize)]
pub struct Opts {
    /// Indicates whether input is a data set.
    ///
    /// If true, dataset will need to be fetched from origin so
    /// input will be interpreted as a [`crate::stream::Data`] object.
    /// Otherwise, input will be provided directly to the model to process.
    is_dataset: bool,
    /// Input for this run.
    input: Bytes,
}
