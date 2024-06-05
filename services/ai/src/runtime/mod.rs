mod deserialize;
mod model;
mod serialize;

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::bail;
use bytes::Bytes;
use lazy_static::lazy_static;
use lightning_utils::config::LIGHTNING_HOME_DIR;
use ort::{TensorElementType, ValueType};

use crate::opts::{BorshVector, Encoding};
use crate::runtime::model::Info;

// Todo: let's improve this.
// Ideally we want every node to run onnx with runtime extensions.
// It might be worthwhile to use a custom build of onnx
// with statically-linked extensions.
lazy_static! {
    static ref ORT_EXTENSIONS_LIB_PATH: PathBuf =
        LIGHTNING_HOME_DIR.join("onnx/libortextensions.dylib");
}

pub struct Session {
    /// The Onnx Runtime Session.
    onnx: ort::Session,
    /// Encoding that will be used for the input and output throughout the session.
    encoding: Encoding,
}

impl Session {
    pub fn new(model: Bytes, encoding: Encoding) -> anyhow::Result<Self> {
        let mut builder = ort::Session::builder()?;
        if ORT_EXTENSIONS_LIB_PATH.exists() {
            builder =
                builder.with_custom_ops_lib(ORT_EXTENSIONS_LIB_PATH.to_string_lossy().as_ref())?
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
                let session_input = deserialize::deserialize_borsh(input)?;
                let session_outputs = self.onnx.run(session_input)?;
                RunOutput::Borsh(serialize::borsh_serialize_outputs(session_outputs)?)
            },
            Encoding::SafeTensors => {
                let session_input = deserialize::deserialize_safetensors(input)?;
                let session_outputs = self.onnx.run(session_input)?;
                RunOutput::SafeTensors(serialize::safetensors_serialize_outputs(session_outputs)?)
            },
        };

        Ok(output)
    }

    pub fn model_info(&self) -> anyhow::Result<Info> {
        let mut name = None;
        let mut description = None;
        let mut producer = None;

        let meta = self.onnx.metadata()?;
        if let Ok(mode_name) = meta.name() {
            name = Some(mode_name);
        }
        if let Ok(desc) = meta.description() {
            description = Some(desc);
        }
        if let Ok(prod) = meta.producer() {
            producer = Some(prod);
        }

        let mut inputs = Vec::new();
        for (i, input) in self.onnx.inputs.iter().enumerate() {
            let input = format!(
                "{i} {}: {}",
                input.name,
                value_type_to_string(&input.input_type)
            );
            inputs.push(input);
        }
        let mut outputs = Vec::new();
        for (i, output) in self.onnx.outputs.iter().enumerate() {
            let output = format!(
                "{i} {}: {}",
                output.name,
                value_type_to_string(&output.output_type)
            );
            outputs.push(output);
        }

        Ok(Info {
            name,
            description,
            producer,
            inputs,
            outputs,
        })
    }
}

pub enum RunOutput {
    SafeTensors(Bytes),
    Borsh(HashMap<String, BorshVector>),
}

fn element_type_to_str(t: TensorElementType) -> &'static str {
    match t {
        TensorElementType::Bfloat16 => "bf16",
        TensorElementType::Bool => "bool",
        TensorElementType::Float16 => "f16",
        TensorElementType::Float32 => "f32",
        TensorElementType::Float64 => "f64",
        TensorElementType::Int16 => "i16",
        TensorElementType::Int32 => "i32",
        TensorElementType::Int64 => "i64",
        TensorElementType::Int8 => "i8",
        TensorElementType::String => "str",
        TensorElementType::Uint16 => "u16",
        TensorElementType::Uint32 => "u32",
        TensorElementType::Uint64 => "u64",
        TensorElementType::Uint8 => "u8",
    }
}

fn value_type_to_string(value: &ValueType) -> String {
    match value {
        ValueType::Tensor { ty, dimensions } => {
            format!("Tensor<{}>({:?})", element_type_to_str(*ty), dimensions)
        },
        ValueType::Map { key, value } => format!(
            "Map<{}, {}>",
            element_type_to_str(*key),
            element_type_to_str(*value)
        ),
        ValueType::Sequence(_) => "Sequence<_>".to_string(),
    }
}
