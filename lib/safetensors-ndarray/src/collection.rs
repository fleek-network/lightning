use std::borrow::Cow;
use std::collections::HashMap;

use ndarray::{ArrayD, IxDyn};
use safetensors::{Dtype, View};

use crate::tensor::Tensor;

pub enum TensorDynType {
    U8(Tensor<u8, IxDyn>),
    U16(Tensor<u16, IxDyn>),
    U32(Tensor<u32, IxDyn>),
    U64(Tensor<u64, IxDyn>),
    I8(Tensor<i8, IxDyn>),
    I16(Tensor<i16, IxDyn>),
    I32(Tensor<i32, IxDyn>),
    I64(Tensor<i64, IxDyn>),
    F32(Tensor<f32, IxDyn>),
    F64(Tensor<f64, IxDyn>),
}

impl View for TensorDynType {
    fn dtype(&self) -> Dtype {
        match self {
            TensorDynType::U8(array) => array.dtype(),
            TensorDynType::U16(array) => array.dtype(),
            TensorDynType::U32(array) => array.dtype(),
            TensorDynType::U64(array) => array.dtype(),
            TensorDynType::I8(array) => array.dtype(),
            TensorDynType::I16(array) => array.dtype(),
            TensorDynType::I32(array) => array.dtype(),
            TensorDynType::I64(array) => array.dtype(),
            TensorDynType::F32(array) => array.dtype(),
            TensorDynType::F64(array) => array.dtype(),
        }
    }

    fn shape(&self) -> &[usize] {
        match self {
            TensorDynType::U8(array) => array.shape(),
            TensorDynType::U16(array) => array.shape(),
            TensorDynType::U32(array) => array.shape(),
            TensorDynType::U64(array) => array.shape(),
            TensorDynType::I8(array) => array.shape(),
            TensorDynType::I16(array) => array.shape(),
            TensorDynType::I32(array) => array.shape(),
            TensorDynType::I64(array) => array.shape(),
            TensorDynType::F32(array) => array.shape(),
            TensorDynType::F64(array) => array.shape(),
        }
    }

    fn data(&self) -> Cow<[u8]> {
        match self {
            TensorDynType::U8(array) => array.data(),
            TensorDynType::U16(array) => array.data(),
            TensorDynType::U32(array) => array.data(),
            TensorDynType::U64(array) => array.data(),
            TensorDynType::I8(array) => array.data(),
            TensorDynType::I16(array) => array.data(),
            TensorDynType::I32(array) => array.data(),
            TensorDynType::I64(array) => array.data(),
            TensorDynType::F32(array) => array.data(),
            TensorDynType::F64(array) => array.data(),
        }
    }

    fn data_len(&self) -> usize {
        match self {
            TensorDynType::U8(array) => array.data_len(),
            TensorDynType::U16(array) => array.data_len(),
            TensorDynType::U32(array) => array.data_len(),
            TensorDynType::U64(array) => array.data_len(),
            TensorDynType::I8(array) => array.data_len(),
            TensorDynType::I16(array) => array.data_len(),
            TensorDynType::I32(array) => array.data_len(),
            TensorDynType::I64(array) => array.data_len(),
            TensorDynType::F32(array) => array.data_len(),
            TensorDynType::F64(array) => array.data_len(),
        }
    }
}

pub struct Collection {
    inner: HashMap<String, TensorDynType>,
}

impl Collection {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    pub fn insert_array_u8(&mut self, name: String, array: ArrayD<u8>) {
        self.inner.insert(name, TensorDynType::U8(Tensor(array)));
    }

    pub fn insert_array_u16(&mut self, name: String, array: ArrayD<u16>) {
        self.inner.insert(name, TensorDynType::U16(Tensor(array)));
    }

    pub fn insert_array_u32(&mut self, name: String, array: ArrayD<u32>) {
        self.inner.insert(name, TensorDynType::U32(Tensor(array)));
    }

    pub fn insert_array_u64(&mut self, name: String, array: ArrayD<u64>) {
        self.inner.insert(name, TensorDynType::U64(Tensor(array)));
    }

    pub fn insert_array_i8(&mut self, name: String, array: ArrayD<i8>) {
        self.inner.insert(name, TensorDynType::I8(Tensor(array)));
    }

    pub fn insert_array_i16(&mut self, name: String, array: ArrayD<i16>) {
        self.inner.insert(name, TensorDynType::I16(Tensor(array)));
    }

    pub fn insert_array_i32(&mut self, name: String, array: ArrayD<i32>) {
        self.inner.insert(name, TensorDynType::I32(Tensor(array)));
    }

    pub fn insert_array_i64(&mut self, name: String, array: ArrayD<i64>) {
        self.inner.insert(name, TensorDynType::I64(Tensor(array)));
    }

    pub fn insert_array_f32(&mut self, name: String, array: ArrayD<f32>) {
        self.inner.insert(name, TensorDynType::F32(Tensor(array)));
    }

    pub fn insert_array_f64(&mut self, name: String, array: ArrayD<f64>) {
        self.inner.insert(name, TensorDynType::F64(Tensor(array)));
    }

    /// Serialize tensors in the collection in safetensors format.
    pub fn serialize(self, data_info: &Option<HashMap<String, String>>) -> anyhow::Result<Vec<u8>> {
        safetensors::serialize(self.inner.into_iter().collect::<Vec<_>>(), data_info)
            .map_err(Into::into)
    }
}
