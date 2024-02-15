//! Utils to convert tensors to Numpy arrays.
use std::io;
use std::io::Cursor;

use anyhow::bail;
use ndarray_npy::WriteNpyExt;
use npyz::{DType, TypeChar};

use crate::tensor::Tensor;

pub fn convert_to_numpy<W: io::Write>(writer: W, tensor: Tensor) -> anyhow::Result<()> {
    match tensor {
        Tensor::Int8(arr) => arr.write_npy(writer)?,
        Tensor::Int16(arr) => arr.write_npy(writer)?,
        Tensor::Int32(arr) => arr.write_npy(writer)?,
        Tensor::Int64(arr) => arr.write_npy(writer)?,
        Tensor::Uint8(arr) => arr.write_npy(writer)?,
        Tensor::Uint16(arr) => arr.write_npy(writer)?,
        Tensor::Uint32(arr) => arr.write_npy(writer)?,
        Tensor::Uint64(arr) => arr.write_npy(writer)?,
        Tensor::Float32(arr) => arr.write_npy(writer)?,
        Tensor::Float64(arr) => arr.write_npy(writer)?,
    }

    Ok(())
}

// Todo: add support for remaining dtypes.
pub fn load_tensor_from_mem(data: &[u8]) -> anyhow::Result<Tensor> {
    let npy_file = npyz::NpyFile::new(Cursor::new(data)).unwrap();
    match npy_file.header().dtype() {
        DType::Plain(ty) => {
            let shape = npy_file.shape().to_vec();
            let order = npy_file.order();

            match ty.type_char() {
                TypeChar::Int => match ty.size_field() {
                    4 => {
                        let data = npy_file.into_vec::<i32>().unwrap();
                        Ok(Tensor::Int32(to_array_d(data, shape, order)))
                    },
                    8 => {
                        let data = npy_file.into_vec::<i64>().unwrap();
                        let arr = to_array_d(data, shape, order);
                        Ok(arr.into())
                    },
                    _ => bail!("dtype `{ty}` is not supported"),
                },
                TypeChar::Float => match ty.size_field() {
                    4 => {
                        let data = npy_file.into_vec::<f32>().unwrap();
                        let arr = to_array_d(data, shape, order);
                        Ok(arr.into())
                    },
                    8 => {
                        let data = npy_file.into_vec::<f64>().unwrap();
                        let arr = to_array_d(data, shape, order);
                        Ok(arr.into())
                    },
                    _ => bail!("dtype `{ty}` is not supported"),
                },
                _ => bail!("dtype `{ty}` is not supported"),
            }
        },
        _ => bail!("numpy inner array and record type not supported"),
    }
}

fn to_array_d<T>(data: Vec<T>, shape: Vec<u64>, order: npyz::Order) -> ndarray::ArrayD<T> {
    use ndarray::ShapeBuilder;

    let shape = shape.into_iter().map(|x| x as usize).collect::<Vec<_>>();
    let true_shape = shape.set_f(order == npyz::Order::Fortran);

    ndarray::ArrayD::from_shape_vec(true_shape, data)
        .unwrap_or_else(|e| panic!("shape error: {}", e))
}
