pub mod numpy;

use std::io::Cursor;
use std::ops::Deref;

use anyhow::bail;
use bytes::Bytes;
use ndarray::Array1;
use ndarray_npy::WriteNpyExt;
use ort::{TensorElementType, Value, ValueType};

use crate::opts::Encoding;

pub fn serialize_value(value: &Value) -> anyhow::Result<(Encoding, Bytes)> {
    let dtype = value.dtype()?;
    let dim = dtype.tensor_dimensions().map(|dim| dim.len()).unwrap_or(0);
    let (encoding, bytes) = match dtype {
        ValueType::Tensor { ty, .. } => match ty {
            TensorElementType::Int8 => {
                let array = value.extract_tensor::<i8>()?.view().deref().to_owned();
                if dim > 1 {
                    let mut buffer = Vec::new();
                    array.write_npy(Cursor::new(&mut buffer))?;
                    (Encoding::Npy, buffer)
                } else {
                    let array = array.into_iter().collect::<Vec<i8>>();
                    (Encoding::BorshInt8, borsh::to_vec::<Vec<i8>>(&array)?)
                }
            },
            TensorElementType::Int16 => {
                let array = value.extract_tensor::<i16>()?.view().deref().to_owned();
                if dim > 1 {
                    let mut buffer = Vec::new();
                    array.write_npy(Cursor::new(&mut buffer))?;
                    (Encoding::Npy, buffer)
                } else {
                    let array = array.into_iter().collect::<Vec<i16>>();
                    (Encoding::BorshInt8, borsh::to_vec::<Vec<i16>>(&array)?)
                }
            },
            TensorElementType::Int32 => {
                let array = value.extract_tensor::<i32>()?.view().deref().to_owned();
                if dim > 1 {
                    let mut buffer = Vec::new();
                    array.write_npy(Cursor::new(&mut buffer))?;
                    (Encoding::Npy, buffer)
                } else {
                    let array = array.into_iter().collect::<Vec<i32>>();
                    (Encoding::BorshInt8, borsh::to_vec::<Vec<i32>>(&array)?)
                }
            },
            TensorElementType::Int64 => {
                let array = value.extract_tensor::<i64>()?.view().deref().to_owned();
                if dim > 1 {
                    let mut buffer = Vec::new();
                    array.write_npy(Cursor::new(&mut buffer))?;
                    (Encoding::Npy, buffer)
                } else {
                    let array = array.into_iter().collect::<Vec<i64>>();
                    (Encoding::BorshInt8, borsh::to_vec::<Vec<i64>>(&array)?)
                }
            },
            TensorElementType::Uint8 => {
                let array = value.extract_tensor::<u8>()?.view().deref().to_owned();
                if dim > 1 {
                    let mut buffer = Vec::new();
                    array.write_npy(Cursor::new(&mut buffer))?;
                    (Encoding::Npy, buffer)
                } else {
                    let array = array.into_iter().collect::<Vec<u8>>();
                    (Encoding::BorshInt8, borsh::to_vec::<Vec<u8>>(&array)?)
                }
            },
            TensorElementType::Uint16 => {
                let array = value.extract_tensor::<u16>()?.view().deref().to_owned();
                if dim > 1 {
                    let mut buffer = Vec::new();
                    array.write_npy(Cursor::new(&mut buffer))?;
                    (Encoding::Npy, buffer)
                } else {
                    let array = array.into_iter().collect::<Vec<u16>>();
                    (Encoding::BorshInt8, borsh::to_vec::<Vec<u16>>(&array)?)
                }
            },
            TensorElementType::Uint32 => {
                let array = value.extract_tensor::<u32>()?.view().deref().to_owned();
                if dim > 1 {
                    let mut buffer = Vec::new();
                    array.write_npy(Cursor::new(&mut buffer))?;
                    (Encoding::Npy, buffer)
                } else {
                    let array = array.into_iter().collect::<Vec<u32>>();
                    (Encoding::BorshInt8, borsh::to_vec::<Vec<u32>>(&array)?)
                }
            },
            TensorElementType::Uint64 => {
                let array = value.extract_tensor::<u64>()?.view().deref().to_owned();
                if dim > 1 {
                    let mut buffer = Vec::new();
                    array.write_npy(Cursor::new(&mut buffer))?;
                    (Encoding::Npy, buffer)
                } else {
                    let array = array.into_iter().collect::<Vec<u64>>();
                    (Encoding::BorshInt8, borsh::to_vec::<Vec<u64>>(&array)?)
                }
            },
            TensorElementType::Float32 => {
                let array = value.extract_tensor::<f32>()?.view().deref().to_owned();
                if dim > 1 {
                    let mut buffer = Vec::new();
                    array.write_npy(Cursor::new(&mut buffer))?;
                    (Encoding::Npy, buffer)
                } else {
                    let array = array.into_iter().collect::<Vec<f32>>();
                    (Encoding::BorshInt8, borsh::to_vec::<Vec<f32>>(&array)?)
                }
            },
            TensorElementType::Float64 => {
                let array = value.extract_tensor::<f64>()?.view().deref().to_owned();
                if dim > 1 {
                    let mut buffer = Vec::new();
                    array.write_npy(Cursor::new(&mut buffer))?;
                    (Encoding::Npy, buffer)
                } else {
                    let array = array.into_iter().collect::<Vec<f64>>();
                    (Encoding::BorshInt8, borsh::to_vec::<Vec<f64>>(&array)?)
                }
            },
            _ => bail!("unsupported value type"),
        },
        _ => bail!("unsupported type for output"),
    };

    Ok((encoding, bytes.into()))
}

pub fn deserialize(format: Encoding, data: Bytes) -> anyhow::Result<Value> {
    if let Encoding::Npy = format {
        return numpy::parse_from_mem(&data);
    }

    match format {
        Encoding::BorshInt8 => Array1::<i8>::from(borsh::from_slice::<Vec<i8>>(data.as_ref())?)
            .try_into()
            .map_err(Into::into),
        Encoding::BorshInt16 => Array1::<i16>::from(borsh::from_slice::<Vec<i16>>(data.as_ref())?)
            .try_into()
            .map_err(Into::into),
        Encoding::BorshInt32 => Array1::<i32>::from(borsh::from_slice::<Vec<i32>>(data.as_ref())?)
            .try_into()
            .map_err(Into::into),
        Encoding::BorshInt64 => Array1::<i64>::from(borsh::from_slice::<Vec<i64>>(data.as_ref())?)
            .try_into()
            .map_err(Into::into),
        Encoding::BorshUint8 => Array1::<u8>::from(borsh::from_slice::<Vec<u8>>(data.as_ref())?)
            .try_into()
            .map_err(Into::into),
        Encoding::BorshUint16 => Array1::<u16>::from(borsh::from_slice::<Vec<u16>>(data.as_ref())?)
            .try_into()
            .map_err(Into::into),
        Encoding::BorshUint32 => Array1::<u32>::from(borsh::from_slice::<Vec<u32>>(data.as_ref())?)
            .try_into()
            .map_err(Into::into),
        Encoding::BorshUint64 => Array1::<u64>::from(borsh::from_slice::<Vec<u64>>(data.as_ref())?)
            .try_into()
            .map_err(Into::into),
        Encoding::BorshFloat32 => {
            Array1::<f32>::from(borsh::from_slice::<Vec<f32>>(data.as_ref())?)
                .try_into()
                .map_err(Into::into)
        },
        Encoding::BorshFloat64 => {
            Array1::<f64>::from(borsh::from_slice::<Vec<f64>>(data.as_ref())?)
                .try_into()
                .map_err(Into::into)
        },
        _ => unreachable!(),
    }
}
