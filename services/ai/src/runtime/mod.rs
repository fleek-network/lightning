use std::collections::HashMap;
use std::io::Cursor;
use std::ops::Deref;
use std::path::Path;

use anyhow::{bail, Context};
use bytes::Bytes;
use ndarray::Array1;
use ort::{SessionInputs, TensorElementType, Value, ValueType};

use crate::opts::Encoding;
use crate::tensor::{numpy, Tensor};
use crate::{EncodedArrayExt, Input, Output};

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
                let tensor = decode_tensor(data.0.1, &(data.0.0.try_into()?))?;
                let value: Value = tensor.try_into()?;
                self.onnx.run(SessionInputs::from([value]))?
            },
            Input::Map { data } => {
                let mut session_input: HashMap<String, Value> = HashMap::new();
                for (input_name, encoded_ext) in data.into_iter() {
                    let tensor = decode_tensor(encoded_ext.0.1, &(encoded_ext.0.0.try_into()?))?;
                    session_input.insert(input_name, tensor.try_into()?);
                }
                let session_input: SessionInputs<'static> = session_input.into();
                self.onnx.run(session_input)?
            },
        };

        // Process the outputs.
        let mut output = HashMap::new();
        for (name, value) in session_outputs.deref().iter() {
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

            output.insert(name.to_string(), EncodedArrayExt((encoding as i8, array)));
        }

        Ok(output)
    }
}

#[inline]
fn decode_tensor(data: Bytes, encoding: &Encoding) -> anyhow::Result<Tensor> {
    match encoding {
        Encoding::Raw => Ok(Tensor::Uint8D1(Array1::<u8>::from(data.to_vec()))),
        Encoding::Npy => numpy::load_tensor_from_mem(&data),
        _ => unimplemented!(),
    }
}
