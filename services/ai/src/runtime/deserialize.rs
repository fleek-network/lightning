use std::collections::HashMap;

use anyhow::bail;
use bytes::Bytes;
use ndarray::Array1;
use ort::{SessionInputs, Value};
use safetensors::{Dtype, SafeTensors};

use crate::opts::{BorshNamedVectors, BorshVectorType};

pub fn deserialize_safetensors(data: Bytes) -> anyhow::Result<SessionInputs<'static>> {
    let mut session_input: HashMap<String, Value> = HashMap::new();
    let safetensors = SafeTensors::deserialize(data.as_ref())?;
    for (name, view) in safetensors.tensors() {
        let value: Value = match view.dtype() {
            Dtype::U8 => {
                safetensors_ndarray::utils::deserialize_u8(view.shape(), view.data())?.try_into()?
            },
            Dtype::I8 => {
                safetensors_ndarray::utils::deserialize_i8(view.shape(), view.data())?.try_into()?
            },
            Dtype::I16 => safetensors_ndarray::utils::deserialize_i16(view.shape(), view.data())?
                .try_into()?,
            Dtype::U16 => safetensors_ndarray::utils::deserialize_u16(view.shape(), view.data())?
                .try_into()?,
            Dtype::I32 => safetensors_ndarray::utils::deserialize_i32(view.shape(), view.data())?
                .try_into()?,
            Dtype::U32 => safetensors_ndarray::utils::deserialize_u32(view.shape(), view.data())?
                .try_into()?,
            Dtype::F32 => safetensors_ndarray::utils::deserialize_f32(view.shape(), view.data())?
                .try_into()?,
            Dtype::F64 => safetensors_ndarray::utils::deserialize_f64(view.shape(), view.data())?
                .try_into()?,
            Dtype::I64 => safetensors_ndarray::utils::deserialize_i64(view.shape(), view.data())?
                .try_into()?,
            Dtype::U64 => safetensors_ndarray::utils::deserialize_u64(view.shape(), view.data())?
                .try_into()?,
            unknown => {
                bail!("unsupported dtype for safetensors: {unknown:?}");
            },
        };

        session_input.insert(name, value);
    }

    Ok(session_input.into())
}

pub fn deserialize_borsh(data: Bytes) -> anyhow::Result<SessionInputs<'static>> {
    let named_inputs = serde_json::from_slice::<BorshNamedVectors>(data.as_ref()).unwrap();
    let mut session_inputs = HashMap::new();
    for (name, vector) in named_inputs {
        let value: Value = match vector.dtype {
            BorshVectorType::Int8 => {
                Array1::<i8>::from(borsh::from_slice::<Vec<i8>>(vector.data.as_ref())?)
                    .try_into()?
            },
            BorshVectorType::Int16 => {
                Array1::<i16>::from(borsh::from_slice::<Vec<i16>>(vector.data.as_ref())?)
                    .try_into()?
            },
            BorshVectorType::Int32 => {
                Array1::<i32>::from(borsh::from_slice::<Vec<i32>>(vector.data.as_ref())?)
                    .try_into()?
            },
            BorshVectorType::Int64 => {
                Array1::<i64>::from(borsh::from_slice::<Vec<i64>>(vector.data.as_ref())?)
                    .try_into()?
            },
            BorshVectorType::Uint8 => {
                Array1::<u8>::from(borsh::from_slice::<Vec<u8>>(vector.data.as_ref()).unwrap())
                    .try_into()?
            },
            BorshVectorType::Uint16 => {
                Array1::<u16>::from(borsh::from_slice::<Vec<u16>>(vector.data.as_ref())?)
                    .try_into()?
            },
            BorshVectorType::Uint32 => {
                Array1::<u32>::from(borsh::from_slice::<Vec<u32>>(vector.data.as_ref())?)
                    .try_into()?
            },
            BorshVectorType::Uint64 => {
                Array1::<u64>::from(borsh::from_slice::<Vec<u64>>(vector.data.as_ref())?)
                    .try_into()?
            },
            BorshVectorType::Float32 => {
                Array1::<f32>::from(borsh::from_slice::<Vec<f32>>(vector.data.as_ref())?)
                    .try_into()?
            },
            BorshVectorType::Float64 => {
                Array1::<f64>::from(borsh::from_slice::<Vec<f64>>(vector.data.as_ref())?)
                    .try_into()?
            },
        };

        session_inputs.insert(name, value);
    }

    Ok(session_inputs.into())
}
