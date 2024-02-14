use std::collections::HashMap;
use std::io::Cursor;
use std::ops::Deref;

use anyhow::bail;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use bytes::Bytes;
use ndarray::ArrayD;
use ort::{inputs, ExtractTensorData, SessionOutputs, ValueType};
use serde::{Deserialize, Serialize};

use crate::tensor::{numpy, Tensor};

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
        // Deserialize input.
        let input = numpy::load_tensor_from_mem(input.as_ref())?;

        match input {
            Tensor::Int32(array) => {
                let outputs = self.onnx.run(inputs![array.view()]?)?;
                serialize_session_outputs::<i32>(outputs)
            },
            Tensor::Int64(array) => {
                let outputs = self.onnx.run(inputs![array.view()]?)?;
                serialize_session_outputs::<i64>(outputs)
            },
            Tensor::Uint32(array) => {
                let outputs = self.onnx.run(inputs![array.view()]?)?;
                serialize_session_outputs::<u32>(outputs)
            },
            Tensor::Uint64(array) => {
                let outputs = self.onnx.run(inputs![array.view()]?)?;
                serialize_session_outputs::<u64>(outputs)
            },
            Tensor::Float32(array) => {
                let outputs = self.onnx.run(inputs![array.view()]?)?;
                serialize_session_outputs::<f32>(outputs)
            },
            Tensor::Float64(array) => {
                let outputs = self.onnx.run(inputs![array.view()]?)?;
                serialize_session_outputs::<f64>(outputs)
            },
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct Output {
    pub format: String,
    pub outputs: HashMap<String, String>,
}

fn serialize_session_outputs<T: ExtractTensorData>(
    outputs: SessionOutputs,
) -> anyhow::Result<Output>
where
    Tensor: From<ArrayD<T>>,
{
    let mut result = HashMap::new();
    for (name, value) in outputs.deref().iter() {
        let mut buffer = Vec::new();
        match value.dtype()? {
            ValueType::Tensor { .. } => {
                let tensor = value.extract_tensor::<T>()?;
                numpy::convert_to_numpy(
                    Cursor::new(&mut buffer),
                    tensor.view().deref().to_owned().into(),
                )?;
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
