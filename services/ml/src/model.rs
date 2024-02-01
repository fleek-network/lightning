use serde::{Deserialize, Serialize};

/// Options for the model.
#[derive(Debug, Deserialize, Serialize)]
pub struct Opts {
    /// Name of the model.
    pub name: String,
    /// Load pre-trained weights.
    pub pretrained: bool,
    /// Custom configuration.
    pub custom: Option<CustomOpts>,
}

/// Options for customizing the model.
#[derive(Debug, Deserialize, Serialize)]
pub struct CustomOpts {
    /// Number of classes.
    pub class_count: u32,
}
