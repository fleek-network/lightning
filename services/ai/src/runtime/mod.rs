use std::path::Path;

use anyhow::bail;
use bytes::Bytes;

use crate::opts::Encoding;
use crate::{utils, Input, Output};

// Todo: let's improve this.
// Ideally we want every node to run onnx with runtime extensions.
// It might be worthwhile to use a custom build of onnx
// with statically-linked extensions.
const ORT_EXTENSIONS_LIB_PATH: &str = "~/.lightning/onnx/libortextensions.dylib";

pub struct Session {
    onnx: ort::Session,
}

impl Session {
    pub fn new(model: Bytes) -> anyhow::Result<Self> {
        let mut builder = ort::Session::builder()?;
        if Path::new(ORT_EXTENSIONS_LIB_PATH).exists() {
            builder = builder.with_custom_ops_lib(ORT_EXTENSIONS_LIB_PATH)?
        }
        Ok(Self {
            onnx: builder.with_model_from_memory(model.as_ref())?,
        })
    }

    /// Runs model on the input.
    pub fn run(&self, input: Input) -> anyhow::Result<Output> {
        if input.data.is_empty() {
            bail!("invalid input length");
        }

        // Process input and pass it to the model.
        let encoding = Encoding::try_from(input.encoding)?;
        let session_outputs = if let Encoding::SafeTensors = encoding {
            let session_input = utils::deserialize_safetensors(input.data)?;
            self.onnx.run(session_input)?
        } else {
            let session_input = utils::deserialize_borsh(encoding, input.data)?;
            self.onnx.run(session_input)?
        };

        // Process the outputs.
        let output =
            utils::serialize_outputs(session_outputs, matches!(encoding, Encoding::SafeTensors))?;

        Ok(output.into())
    }
}
