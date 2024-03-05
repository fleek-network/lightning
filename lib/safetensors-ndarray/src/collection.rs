use std::borrow::Cow;
use std::collections::HashMap;

use ndarray::{ArrayD, IxDyn};
use safetensors::{Dtype, SafeTensorError, View};

use crate::tensor::Tensor;

enum Wrapper {
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

impl View for Wrapper {
    fn dtype(&self) -> Dtype {
        match self {
            Wrapper::U8(array) => array.dtype(),
            Wrapper::U16(array) => array.dtype(),
            Wrapper::U32(array) => array.dtype(),
            Wrapper::U64(array) => array.dtype(),
            Wrapper::I8(array) => array.dtype(),
            Wrapper::I16(array) => array.dtype(),
            Wrapper::I32(array) => array.dtype(),
            Wrapper::I64(array) => array.dtype(),
            Wrapper::F32(array) => array.dtype(),
            Wrapper::F64(array) => array.dtype(),
        }
    }

    fn shape(&self) -> &[usize] {
        match self {
            Wrapper::U8(array) => array.shape(),
            Wrapper::U16(array) => array.shape(),
            Wrapper::U32(array) => array.shape(),
            Wrapper::U64(array) => array.shape(),
            Wrapper::I8(array) => array.shape(),
            Wrapper::I16(array) => array.shape(),
            Wrapper::I32(array) => array.shape(),
            Wrapper::I64(array) => array.shape(),
            Wrapper::F32(array) => array.shape(),
            Wrapper::F64(array) => array.shape(),
        }
    }

    fn data(&self) -> Cow<[u8]> {
        match self {
            Wrapper::U8(array) => array.data(),
            Wrapper::U16(array) => array.data(),
            Wrapper::U32(array) => array.data(),
            Wrapper::U64(array) => array.data(),
            Wrapper::I8(array) => array.data(),
            Wrapper::I16(array) => array.data(),
            Wrapper::I32(array) => array.data(),
            Wrapper::I64(array) => array.data(),
            Wrapper::F32(array) => array.data(),
            Wrapper::F64(array) => array.data(),
        }
    }

    fn data_len(&self) -> usize {
        match self {
            Wrapper::U8(array) => array.data_len(),
            Wrapper::U16(array) => array.data_len(),
            Wrapper::U32(array) => array.data_len(),
            Wrapper::U64(array) => array.data_len(),
            Wrapper::I8(array) => array.data_len(),
            Wrapper::I16(array) => array.data_len(),
            Wrapper::I32(array) => array.data_len(),
            Wrapper::I64(array) => array.data_len(),
            Wrapper::F32(array) => array.data_len(),
            Wrapper::F64(array) => array.data_len(),
        }
    }
}

pub struct Collection {
    inner: HashMap<String, Wrapper>,
}

impl Collection {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    pub fn insert_array_u8(&mut self, name: String, array: ArrayD<u8>) {
        self.inner.insert(name, Wrapper::U8(Tensor(array)));
    }

    pub fn insert_array_u16(&mut self, name: String, array: ArrayD<u16>) {
        self.inner.insert(name, Wrapper::U16(Tensor(array)));
    }

    pub fn insert_array_u32(&mut self, name: String, array: ArrayD<u32>) {
        self.inner.insert(name, Wrapper::U32(Tensor(array)));
    }

    pub fn insert_array_u64(&mut self, name: String, array: ArrayD<u64>) {
        self.inner.insert(name, Wrapper::U64(Tensor(array)));
    }

    pub fn insert_array_i8(&mut self, name: String, array: ArrayD<i8>) {
        self.inner.insert(name, Wrapper::I8(Tensor(array)));
    }

    pub fn insert_array_i16(&mut self, name: String, array: ArrayD<i16>) {
        self.inner.insert(name, Wrapper::I16(Tensor(array)));
    }

    pub fn insert_array_i32(&mut self, name: String, array: ArrayD<i32>) {
        self.inner.insert(name, Wrapper::I32(Tensor(array)));
    }

    pub fn insert_array_i64(&mut self, name: String, array: ArrayD<i64>) {
        self.inner.insert(name, Wrapper::I64(Tensor(array)));
    }

    pub fn insert_array_f32(&mut self, name: String, array: ArrayD<f32>) {
        self.inner.insert(name, Wrapper::F32(Tensor(array)));
    }

    pub fn insert_array_f64(&mut self, name: String, array: ArrayD<f64>) {
        self.inner.insert(name, Wrapper::F64(Tensor(array)));
    }

    /// Serialize tensors in the collection in safetensors format.
    pub fn serialize(
        self,
        data_info: &Option<HashMap<String, String>>,
    ) -> Result<Vec<u8>, SafeTensorError> {
        safetensors::serialize(self.inner.into_iter().collect::<Vec<_>>(), data_info)
    }
}
