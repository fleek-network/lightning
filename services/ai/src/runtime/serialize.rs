use std::collections::HashMap;
use std::ops::Deref;

use anyhow::bail;
use bytes::Bytes;
use ort::{SessionOutputs, TensorElementType, ValueType};
use safetensors_ndarray::collection::Collection;

use crate::opts::{BorshNamedVectors, BorshVector, BorshVectorType};

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

    safetensors_out.serialize(&None).map(Into::into)
}

pub fn borsh_serialize_outputs(
    session_outputs: SessionOutputs,
) -> anyhow::Result<BorshNamedVectors> {
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
                                data: borsh::to_vec::<Vec<i8>>(&array)?,
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
                                data: borsh::to_vec::<Vec<i16>>(&array)?,
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
                                data: borsh::to_vec::<Vec<i32>>(&array)?,
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
                                data: borsh::to_vec::<Vec<i64>>(&array)?,
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
                                data: borsh::to_vec::<Vec<u8>>(&array)?,
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
                                data: borsh::to_vec::<Vec<u16>>(&array)?,
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
                                data: borsh::to_vec::<Vec<u32>>(&array)?,
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
                                data: borsh::to_vec::<Vec<u64>>(&array)?,
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
                                data: borsh::to_vec::<Vec<f32>>(&array)?,
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
                                data: borsh::to_vec::<Vec<f64>>(&array)?,
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
