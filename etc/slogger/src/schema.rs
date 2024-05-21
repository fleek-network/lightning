use std::string::ToString;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Record {
    metadata: Metadata,
    args: String,
    module_path: Option<String>,
    file: Option<String>,
    line: Option<u32>,
}

#[derive(Deserialize, Serialize)]
pub struct Metadata {
    level: String,
    target: String,
}

impl From<&log::Record<'_>> for Record {
    fn from(value: &log::Record) -> Self {
        Self {
            metadata: Metadata {
                level: value.metadata().level().to_string(),
                target: value.metadata().target().to_string(),
            },
            args: value.args().to_string(),
            module_path: value.module_path().map(|path| path.to_string()),
            file: value.file().map(|f| f.to_string()),
            line: value.line(),
        }
    }
}

