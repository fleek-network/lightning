use ndarray::{Array, Dimension};
use safetensors::{Dtype, View};
use std::borrow::Cow;


pub struct Tensor<A, D>(Array<A, D>);

impl<A, D> Tensor<A, D>
where
    D: Dimension
{
    fn buffer(&self) -> &[u8] {
        let slice = self.0.as_slice().expect("Non contiguous tensors");
        let num_bytes = std::mem::size_of::<A>();
        let new_slice: &[u8] = unsafe {
            std::slice::from_raw_parts(slice.as_ptr() as *const u8, slice.len() * num_bytes)
        };
        new_slice
    }
}

macro_rules! impl_view {
    ($type:ty, $dtype:expr) => {
        impl<D: Dimension> View for Tensor<$type, D> {
            fn dtype(&self) -> Dtype {
                $dtype
            }

            fn shape(&self) -> &[usize] {
                self.0.shape()
            }

            fn data(&self) -> Cow<[u8]> {
                self.buffer().into()
            }

            fn data_len(&self) -> usize {
                self.buffer().len()
            }
        }
    };
}

impl_view!(u8, Dtype::U8);
impl_view!(u16, Dtype::U16);
impl_view!(u32, Dtype::U32);
impl_view!(u64, Dtype::U64);
impl_view!(i8, Dtype::I8);
impl_view!(i16, Dtype::I16);
impl_view!(i32, Dtype::I32);
impl_view!(i64, Dtype::I64);
impl_view!(f32, Dtype::F32);
impl_view!(f64, Dtype::F64);

