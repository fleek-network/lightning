use std::collections::HashMap;
use std::io::Cursor;
use std::ops::Deref;

use anyhow::{bail, Context};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use bytes::Bytes;
use ort::{SessionInputs, SessionOutputs, TensorElementType, Value, ValueType};
use serde::{Deserialize, Serialize};

use crate::tensor::numpy;
use crate::{Encoding, Output};

pub struct Session {
    onnx: ort::Session,
}

impl Session {
    pub fn new(model: Bytes) -> anyhow::Result<Self> {
        let session = ort::Session::builder()?.with_model_from_memory(model.as_ref())?;
        Ok(Self { onnx: session })
    }

    /// Runs model on the input.
    pub fn run(&self, input: Bytes) -> anyhow::Result<Output> {
        let input = serde_json::from_slice::<Input>(input.as_ref())
            .context("failed to deserialize input")?;
        let outputs = match input {
            Input::Raw(input) => {
                let tensor = numpy::load_tensor_from_mem(&input)?;
                let value: Value = tensor.try_into()?;
                self.onnx.run(SessionInputs::from([value]))?
            },
            Input::Map(input) => {
                let session_input = process_map_input(input)?;
                self.onnx.run(session_input)?
            },
        };
        serialize_session_outputs(outputs)
    }
}

fn serialize_session_outputs(outputs: SessionOutputs) -> anyhow::Result<Output> {
    let mut result = HashMap::new();
    for (name, value) in outputs.deref().iter() {
        let mut buffer = Vec::new();
        match value.dtype()? {
            ValueType::Tensor { ty, .. } => {
                let tensor = match ty {
                    TensorElementType::Int8 => value
                        .extract_tensor::<i8>()?
                        .view()
                        .deref()
                        .to_owned()
                        .into(),
                    TensorElementType::Int16 => value
                        .extract_tensor::<i16>()?
                        .view()
                        .deref()
                        .to_owned()
                        .into(),
                    TensorElementType::Int32 => value
                        .extract_tensor::<i32>()?
                        .view()
                        .deref()
                        .to_owned()
                        .into(),
                    TensorElementType::Int64 => value
                        .extract_tensor::<i64>()?
                        .view()
                        .deref()
                        .to_owned()
                        .into(),
                    TensorElementType::Uint8 => value
                        .extract_tensor::<u8>()?
                        .view()
                        .deref()
                        .to_owned()
                        .into(),
                    TensorElementType::Uint16 => value
                        .extract_tensor::<u16>()?
                        .view()
                        .deref()
                        .to_owned()
                        .into(),
                    TensorElementType::Uint32 => value
                        .extract_tensor::<u32>()?
                        .view()
                        .deref()
                        .to_owned()
                        .into(),
                    TensorElementType::Uint64 => value
                        .extract_tensor::<u64>()?
                        .view()
                        .deref()
                        .to_owned()
                        .into(),
                    TensorElementType::Float32 => value
                        .extract_tensor::<f32>()?
                        .view()
                        .deref()
                        .to_owned()
                        .into(),
                    TensorElementType::Float64 => value
                        .extract_tensor::<f64>()?
                        .view()
                        .deref()
                        .to_owned()
                        .into(),
                    _ => bail!("unsupported value type"),
                };
                numpy::convert_to_numpy(Cursor::new(&mut buffer), tensor)?;
            },
            _ => bail!("unsupported type for output"),
        }
        result.insert(name.to_string(), BASE64_STANDARD.encode(buffer));
    }

    Ok(Output {
        encoding: Encoding::Npy,
        outputs: result,
    })
}

#[derive(Deserialize, Serialize)]
pub enum Input {
    Raw(Bytes),
    Map(HashMap<String, Bytes>),
}

fn process_map_input(input: HashMap<String, Bytes>) -> anyhow::Result<SessionInputs<'static>> {
    let mut mapped_values: HashMap<String, Value> = HashMap::new();
    for (input_name, bytes) in input.into_iter() {
        let tensor = numpy::load_tensor_from_mem(&bytes)?;
        mapped_values.insert(input_name, tensor.try_into()?);
    }
    Ok(mapped_values.into())
}
