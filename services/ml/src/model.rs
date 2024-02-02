use std::str::FromStr;

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

#[derive(Debug, Deserialize, Serialize)]
#[repr(u8)]
pub enum Model {
    Resnet18,
    Resnet34,
}

impl FromStr for Model {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let model = match s {
            "resnet18" => Self::Resnet18,
            "resnet34" => Self::Resnet34,
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "unknown model",
                ));
            },
        };

        Ok(model)
    }
}
