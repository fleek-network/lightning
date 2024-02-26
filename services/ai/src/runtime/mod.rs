use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;

use anyhow::Context;
use bytes::Bytes;
use ort::{SessionInputs, Value};

use crate::{array, EncodedArrayExt, Input, Output};

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
    pub fn run(&self, input: Bytes) -> anyhow::Result<Output> {
        let input = rmp_serde::from_slice::<Input>(input.as_ref())
            .context("failed to deserialize input")?;

        // Process input and pass it to the model.
        let session_outputs = match input {
            Input::Array { data } => {
                let value = array::deserialize(data.0.0.try_into()?, data.0.1)?;
                self.onnx.run(SessionInputs::from([value]))?
            },
            Input::Map { data } => {
                let mut session_input: HashMap<String, Value> = HashMap::new();
                for (input_name, encoded_ext) in data.into_iter() {
                    let value = array::deserialize(encoded_ext.0.0.try_into()?, encoded_ext.0.1)?;
                    session_input.insert(input_name, value);
                }
                let session_input: SessionInputs<'static> = session_input.into();
                self.onnx.run(session_input)?
            },
        };

        // Process the outputs.
        let mut output = HashMap::new();
        for (name, value) in session_outputs.deref().iter() {
            // Todo: let's allow users to force the output type for npy.
            let (encoding, bytes) = array::serialize_value(value)?;
            output.insert(name.to_string(), EncodedArrayExt((encoding as i8, bytes)));
        }

        Ok(output)
    }
}
