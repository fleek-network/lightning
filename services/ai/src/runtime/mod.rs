use std::collections::HashMap;
use std::io::Cursor;
use std::ops::Deref;

use anyhow::bail;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use bytes::Bytes;
use ort::{SessionInputs, SessionOutputs, TensorElementType, Value, ValueType};
use serde::{Deserialize, Serialize};

use crate::tensor::numpy;

pub struct Session {
    onnx: ort::Session,
}

impl Session {
    pub fn new(model: Bytes) -> anyhow::Result<Self> {
        let session = ort::Session::builder()?.with_model_from_memory(model.as_ref())?;
        for (i, input) in session.inputs.iter().enumerate() {
            println!(
                "    {i} {}: {}",
                input.name,
                display_value_type(&input.input_type)
            );
        }
        Ok(Self { onnx: session })
    }

    /// Runs model on the input.
    pub fn run(&self, input: Bytes) -> anyhow::Result<Output> {
        let input = serde_json::from_slice::<Input>(input.as_ref())?;
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

#[derive(Deserialize, Serialize)]
pub struct Output {
    pub format: String,
    pub outputs: HashMap<String, String>,
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
        format: "npy".to_string(),
        outputs: result,
    })
}

fn display_value_type(value: &ValueType) -> String {
    match value {
        ValueType::Tensor { ty, dimensions } => {
            format!(
                "Tensor<{}>({})",
                display_element_type(*ty),
                dimensions
                    .iter()
                    .map(|c| if *c == -1 {
                        "dyn".to_string()
                    } else {
                        c.to_string()
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        },
        ValueType::Map { key, value } => format!(
            "Map<{}, {}>",
            display_element_type(*key),
            display_element_type(*value)
        ),
        ValueType::Sequence(inner) => format!("Sequence<{}>", display_value_type(inner)),
    }
}

fn display_element_type(t: TensorElementType) -> &'static str {
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
    mapped_values.try_into().map_err(Into::into)
}
