pub mod numpy;

use ndarray::ArrayD;

// Todo: add support for remaining types.
pub enum Tensor {
    Int32(ArrayD<i32>),
    Int64(ArrayD<i64>),
    Uint32(ArrayD<u32>),
    Uint64(ArrayD<u64>),
    Float32(ArrayD<f32>),
    Float64(ArrayD<f64>),
}

impl From<ArrayD<i32>> for Tensor {
    fn from(value: ArrayD<i32>) -> Self {
        Self::Int32(value)
    }
}

impl From<ArrayD<i64>> for Tensor {
    fn from(value: ArrayD<i64>) -> Self {
        Self::Int64(value)
    }
}

impl From<ArrayD<f32>> for Tensor {
    fn from(value: ArrayD<f32>) -> Self {
        Self::Float32(value)
    }
}

impl From<ArrayD<f64>> for Tensor {
    fn from(value: ArrayD<f64>) -> Self {
        Self::Float64(value)
    }
}

impl From<ArrayD<u32>> for Tensor {
    fn from(value: ArrayD<u32>) -> Self {
        Self::Uint32(value)
    }
}

impl From<ArrayD<u64>> for Tensor {
    fn from(value: ArrayD<u64>) -> Self {
        Self::Uint64(value)
    }
}
