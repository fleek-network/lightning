use std::collections::HashMap;
use std::convert::TryInto;
use std::ops::Deref;

use anyhow::bail;
use bytes::Bytes;
use ndarray::Array1;
use ort::{SessionInputs, SessionOutputs, TensorElementType, Value, ValueType};
use safetensors::tensor::{Dtype, SafeTensors};
use safetensors_ndarray::collection::Collection;

use crate::opts::{BorshInput, BorshVector, BorshVectorType};

pub fn safetensors_serialize_outputs(session_outputs: SessionOutputs) -> anyhow::Result<Bytes> {
    let mut safetensors_out = Collection::new();
    for (name, value) in session_outputs.deref().iter() {
        let dtype = value.dtype()?;
        match dtype {
            ValueType::Tensor { ty, .. } => match ty {
                TensorElementType::Int8 => {
                    let array = value.extract_tensor::<i8>()?.view().deref().to_owned();
                    safetensors_out.insert_array_i8(name.to_string(), array);
                },
                TensorElementType::Int16 => {
                    let array = value.extract_tensor::<i16>()?.view().deref().to_owned();
                    safetensors_out.insert_array_i16(name.to_string(), array);
                },
                TensorElementType::Int32 => {
                    let array = value.extract_tensor::<i32>()?.view().deref().to_owned();
                    safetensors_out.insert_array_i32(name.to_string(), array);
                },
                TensorElementType::Int64 => {
                    let array = value.extract_tensor::<i64>()?.view().deref().to_owned();
                    safetensors_out.insert_array_i64(name.to_string(), array);
                },
                TensorElementType::Uint8 => {
                    let array = value.extract_tensor::<u8>()?.view().deref().to_owned();

                    safetensors_out.insert_array_u8(name.to_string(), array);
                },
                TensorElementType::Uint16 => {
                    let array = value.extract_tensor::<u16>()?.view().deref().to_owned();

                    safetensors_out.insert_array_u16(name.to_string(), array);
                },
                TensorElementType::Uint32 => {
                    let array = value.extract_tensor::<u32>()?.view().deref().to_owned();
                    safetensors_out.insert_array_u32(name.to_string(), array);
                },
                TensorElementType::Uint64 => {
                    let array = value.extract_tensor::<u64>()?.view().deref().to_owned();
                    safetensors_out.insert_array_u64(name.to_string(), array);
                },
                TensorElementType::Float32 => {
                    let array = value.extract_tensor::<f32>()?.view().deref().to_owned();
                    safetensors_out.insert_array_f32(name.to_string(), array);
                },
                TensorElementType::Float64 => {
                    let array = value.extract_tensor::<f64>()?.view().deref().to_owned();
                    safetensors_out.insert_array_f64(name.to_string(), array);
                },
                _ => bail!("unsupported value type"),
            },
            _ => bail!("unsupported type for output"),
        }
    }

    safetensors_out
        .serialize(&None)
        .map(Into::into)
        .map_err(Into::into)
}

pub fn borsh_serialize_outputs(
    session_outputs: SessionOutputs,
) -> anyhow::Result<HashMap<String, BorshVector>> {
    let mut borsh_out = HashMap::new();
    for (name, value) in session_outputs.deref().iter() {
        let name = name.to_string();
        let dtype = value.dtype()?;
        let dim = dtype.tensor_dimensions().map(|dim| dim.len()).unwrap_or(0);
        match dtype {
            ValueType::Tensor { ty, .. } => match ty {
                TensorElementType::Int8 => {
                    let array = value.extract_tensor::<i8>()?.view().deref().to_owned();
                    if dim != 1 {
                        bail!("cannot serialize array with dim {dim:?} using borsh");
                    } else {
                        let array = array.into_iter().collect::<Vec<i8>>();
                        borsh_out.insert(
                            name,
                            BorshVector {
                                dtype: BorshVectorType::Int8,
                                data: borsh::to_vec::<Vec<i8>>(&array)?.into(),
                            },
                        );
                    }
                },
                TensorElementType::Int16 => {
                    let array = value.extract_tensor::<i16>()?.view().deref().to_owned();
                    if dim > 1 {
                        bail!("cannot serialize array with dim {dim:?} using borsh");
                    } else {
                        let array = array.into_iter().collect::<Vec<i16>>();
                        borsh_out.insert(
                            name,
                            BorshVector {
                                dtype: BorshVectorType::Int8,
                                data: borsh::to_vec::<Vec<i16>>(&array)?.into(),
                            },
                        );
                    }
                },
                TensorElementType::Int32 => {
                    let array = value.extract_tensor::<i32>()?.view().deref().to_owned();
                    if dim > 1 {
                        bail!("cannot serialize array with dim {dim:?} using borsh");
                    } else {
                        let array = array.into_iter().collect::<Vec<i32>>();
                        borsh_out.insert(
                            name,
                            BorshVector {
                                dtype: BorshVectorType::Int8,
                                data: borsh::to_vec::<Vec<i32>>(&array)?.into(),
                            },
                        );
                    }
                },
                TensorElementType::Int64 => {
                    let array = value.extract_tensor::<i64>()?.view().deref().to_owned();
                    if dim > 1 {
                        bail!("cannot serialize array with dim {dim:?} using borsh");
                    } else {
                        let array = array.into_iter().collect::<Vec<i64>>();
                        borsh_out.insert(
                            name,
                            BorshVector {
                                dtype: BorshVectorType::Int8,
                                data: borsh::to_vec::<Vec<i64>>(&array)?.into(),
                            },
                        );
                    }
                },
                TensorElementType::Uint8 => {
                    let array = value.extract_tensor::<u8>()?.view().deref().to_owned();
                    if dim > 1 {
                        bail!("cannot serialize array with dim {dim:?} using borsh");
                    } else {
                        let array = array.into_iter().collect::<Vec<u8>>();
                        borsh_out.insert(
                            name,
                            BorshVector {
                                dtype: BorshVectorType::Int8,
                                data: borsh::to_vec::<Vec<u8>>(&array)?.into(),
                            },
                        );
                    }
                },
                TensorElementType::Uint16 => {
                    let array = value.extract_tensor::<u16>()?.view().deref().to_owned();
                    if dim > 1 {
                        bail!("cannot serialize array with dim {dim:?} using borsh");
                    } else {
                        let array = array.into_iter().collect::<Vec<u16>>();
                        borsh_out.insert(
                            name,
                            BorshVector {
                                dtype: BorshVectorType::Int8,
                                data: borsh::to_vec::<Vec<u16>>(&array)?.into(),
                            },
                        );
                    }
                },
                TensorElementType::Uint32 => {
                    let array = value.extract_tensor::<u32>()?.view().deref().to_owned();
                    if dim > 1 {
                        bail!("cannot serialize array with dim {dim:?} using borsh");
                    } else {
                        let array = array.into_iter().collect::<Vec<u32>>();
                        borsh_out.insert(
                            name,
                            BorshVector {
                                dtype: BorshVectorType::Int8,
                                data: borsh::to_vec::<Vec<u32>>(&array)?.into(),
                            },
                        );
                    }
                },
                TensorElementType::Uint64 => {
                    let array = value.extract_tensor::<u64>()?.view().deref().to_owned();
                    if dim > 1 {
                        bail!("cannot serialize array with dim {dim:?} using borsh");
                    } else {
                        let array = array.into_iter().collect::<Vec<u64>>();
                        borsh_out.insert(
                            name,
                            BorshVector {
                                dtype: BorshVectorType::Uint64,
                                data: borsh::to_vec::<Vec<u64>>(&array)?.into(),
                            },
                        );
                    }
                },
                TensorElementType::Float32 => {
                    let array = value.extract_tensor::<f32>()?.view().deref().to_owned();
                    if dim > 1 {
                        bail!("cannot serialize array with dim {dim:?} using borsh");
                    } else {
                        let array = array.into_iter().collect::<Vec<f32>>();
                        borsh_out.insert(
                            name,
                            BorshVector {
                                dtype: BorshVectorType::Int8,
                                data: borsh::to_vec::<Vec<f32>>(&array)?.into(),
                            },
                        );
                    }
                },
                TensorElementType::Float64 => {
                    let array = value.extract_tensor::<f64>()?.view().deref().to_owned();
                    if dim > 1 {
                        bail!("cannot serialize array with dim {dim:?} using borsh");
                    } else {
                        let array = array.into_iter().collect::<Vec<f64>>();
                        borsh_out.insert(
                            name,
                            BorshVector {
                                dtype: BorshVectorType::Int8,
                                data: borsh::to_vec::<Vec<f64>>(&array)?.into(),
                            },
                        );
                    }
                },
                _ => bail!("unsupported value type"),
            },
            _ => bail!("unsupported type for output"),
        }
    }

    Ok(borsh_out)
}

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
    let named_inputs = serde_json::from_slice::<BorshInput>(data.as_ref())?;
    let mut session_inputs = HashMap::new();
    for (name, vector) in named_inputs {
        let value: Value = match vector.dtype {
            BorshVectorType::Int8 => {
                Array1::<i8>::from(borsh::from_slice::<Vec<i8>>(vector.data.as_ref())?)
                    .try_into()?
            },
            BorshVectorType::Int16 => {
                Array1::<i16>::from(borsh::from_slice::<Vec<i16>>(data.as_ref())?).try_into()?
            },
            BorshVectorType::Int32 => {
                Array1::<i32>::from(borsh::from_slice::<Vec<i32>>(data.as_ref())?).try_into()?
            },
            BorshVectorType::Int64 => {
                Array1::<i64>::from(borsh::from_slice::<Vec<i64>>(data.as_ref())?).try_into()?
            },
            BorshVectorType::Uint8 => {
                Array1::<u8>::from(borsh::from_slice::<Vec<u8>>(data.as_ref())?).try_into()?
            },
            BorshVectorType::Uint16 => {
                Array1::<u16>::from(borsh::from_slice::<Vec<u16>>(data.as_ref())?).try_into()?
            },
            BorshVectorType::Uint32 => {
                Array1::<u32>::from(borsh::from_slice::<Vec<u32>>(data.as_ref())?).try_into()?
            },
            BorshVectorType::Uint64 => {
                Array1::<u64>::from(borsh::from_slice::<Vec<u64>>(data.as_ref())?).try_into()?
            },
            BorshVectorType::Float32 => {
                Array1::<f32>::from(borsh::from_slice::<Vec<f32>>(data.as_ref())?).try_into()?
            },
            BorshVectorType::Float64 => {
                Array1::<f64>::from(borsh::from_slice::<Vec<f64>>(data.as_ref())?).try_into()?
            },
        };

        session_inputs.insert(name, value);
    }

    Ok(session_inputs.into())
}
