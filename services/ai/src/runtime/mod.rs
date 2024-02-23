use std::collections::HashMap;
use std::io::Cursor;
use std::ops::Deref;

use anyhow::{bail, Context};
use bytes::Bytes;
use ndarray::Array1;
use ort::{SessionInputs, SessionOutputs, TensorElementType, Value, ValueType};

use crate::tensor::{numpy, Tensor};
use crate::{EncodedArrayExt, Encoding, Input, Output};

pub struct Session {
    onnx: ort::Session,
}

impl Session {
    pub fn new(model: Bytes) -> anyhow::Result<Self> {
        let session = ort::Session::builder()?
            .with_custom_ops_lib("/Users/acadia/Repo/onnxruntime-extensions/out/Darwin/RelWithDebInfo/lib/libortextensions.dylib")?
            .with_model_from_memory(model.as_ref())?;
        Ok(Self { onnx: session })
    }

    /// Runs model on the input.
    pub fn run(&self, input: Bytes) -> anyhow::Result<Output> {
        let input = rmp_serde::from_slice::<Input>(input.as_ref())
            .context("failed to deserialize input")?;
        let (encoding, outputs) = match input {
            Input::Array { encoding, data } => {
                let tensor = decode_tensor(data, &encoding)?;
                let value: Value = tensor.try_into()?;
                (encoding, self.onnx.run(SessionInputs::from([value]))?)
            },
            Input::Map { encoding, data } => {
                let mut session_input: HashMap<String, Value> = HashMap::new();
                for (input_name, bytes) in data.into_iter() {
                    let tensor = decode_tensor(bytes, &encoding)?;
                    session_input.insert(input_name, tensor.try_into()?);
                }
                let session_input: SessionInputs<'static> = session_input.into();
                (encoding, self.onnx.run(session_input)?)
            },
        };
        serialize_session_outputs(outputs, encoding)
    }
}

fn serialize_session_outputs(
    outputs: SessionOutputs,
    encoding: Encoding,
) -> anyhow::Result<Output> {
    let mut result = HashMap::new();
    for (name, value) in outputs.deref().iter() {
        let dtype = value.dtype()?;
        let dim = dtype.tensor_dimensions().map(|dim| dim.len()).unwrap_or(0);
        let tensor: Tensor = match dtype {
            ValueType::Tensor { ty, .. } => match ty {
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
            },
            _ => bail!("unsupported type for output"),
        };

        let (encoding, array) = if dim > 1 {
            let mut buffer = Vec::new();
            numpy::convert_to_numpy(Cursor::new(&mut buffer), tensor)?;
            (Encoding::Npy, buffer.into())
        } else {
            let array: Vec<u8> = tensor.try_into()?;
            (Encoding::Borsh, array.into())
        };

        result.insert(name.to_string(), EncodedArrayExt((encoding as i8, array)));
    }

    Ok(Output {
        encoding,
        outputs: result,
    })
}

#[inline]
fn decode_tensor(data: Bytes, encoding: &Encoding) -> anyhow::Result<Tensor> {
    match encoding {
        Encoding::Raw => Ok(Tensor::Uint8D1(Array1::<u8>::from(data.to_vec()))),
        Encoding::Npy => numpy::load_tensor_from_mem(&data),
        _ => unimplemented!(),
    }
}
