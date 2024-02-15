pub mod numpy;

use ndarray::ArrayD;

// Todo: add support for remaining types.
pub enum Tensor {
    Int8(ArrayD<i8>),
    Int16(ArrayD<i16>),
    Int32(ArrayD<i32>),
    Int64(ArrayD<i64>),
    Uint8(ArrayD<u8>),
    Uint16(ArrayD<u16>),
    Uint32(ArrayD<u32>),
    Uint64(ArrayD<u64>),
    Float32(ArrayD<f32>),
    Float64(ArrayD<f64>),
}

impl From<ArrayD<i8>> for Tensor {
    fn from(value: ArrayD<i8>) -> Self {
        Self::Int8(value)
    }
}

impl From<ArrayD<i16>> for Tensor {
    fn from(value: ArrayD<i16>) -> Self {
        Self::Int16(value)
    }
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

impl From<ArrayD<u8>> for Tensor {
    fn from(value: ArrayD<u8>) -> Self {
        Self::Uint8(value)
    }
}

impl From<ArrayD<u16>> for Tensor {
    fn from(value: ArrayD<u16>) -> Self {
        Self::Uint16(value)
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
