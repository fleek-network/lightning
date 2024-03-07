use std::collections::HashMap;
use std::path::Path;

use anyhow::bail;
use bytes::Bytes;

use crate::opts::{BorshVector, Encoding};
use crate::utils;

// Todo: let's improve this.
// Ideally we want every node to run onnx with runtime extensions.
// It might be worthwhile to use a custom build of onnx
// with statically-linked extensions.
const ORT_EXTENSIONS_LIB_PATH: &str = "~/.lightning/onnx/libortextensions.dylib";

pub struct Session {
    /// The Onnx Runtime Session.
    onnx: ort::Session,
    /// Encoding that will be used for the input and output throughout the session.
    encoding: Encoding,
}

impl Session {
    pub fn new(model: Bytes, encoding: Encoding) -> anyhow::Result<Self> {
        let mut builder = ort::Session::builder()?;
        if Path::new(ORT_EXTENSIONS_LIB_PATH).exists() {
            builder = builder.with_custom_ops_lib(ORT_EXTENSIONS_LIB_PATH)?
        }
        Ok(Self {
            onnx: builder.with_model_from_memory(model.as_ref())?,
            encoding,
        })
    }

    /// Runs model on the input.
    pub fn run(&self, input: Bytes) -> anyhow::Result<RunOutput> {
        if input.is_empty() {
            bail!("invalid input length");
        }

        // Process input and pass it to the model.
        let output = match self.encoding {
            Encoding::Borsh => {
                let session_input = utils::deserialize_borsh(input)?;
                let session_outputs = self.onnx.run(session_input)?;
                RunOutput::Borsh(utils::borsh_serialize_outputs(session_outputs)?)
            },
            Encoding::SafeTensors => {
                let session_input = utils::deserialize_safetensors(input)?;
                let session_outputs = self.onnx.run(session_input)?;
                RunOutput::SafeTensors(utils::safetensors_serialize_outputs(session_outputs)?)
            },
        };

        Ok(output)
    }
}

pub enum RunOutput {
    SafeTensors(Bytes),
    Borsh(HashMap<String, BorshVector>),
}
