pub mod numpy;

use ndarray::{Array1, ArrayD};
use ort::Value;

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
    Uint8D1(Array1<u8>),
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

impl From<Array1<u8>> for Tensor {
    fn from(value: Array1<u8>) -> Self {
        Self::Uint8D1(value)
    }
}

impl TryFrom<Tensor> for Vec<u8> {
    type Error = std::io::Error;

    fn try_from(value: Tensor) -> Result<Self, Self::Error> {
        match value {
            Tensor::Int8(array) => {
                if array.ndim() > 1 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "cannot flatten a multidimensional array",
                    ));
                }
                let array = array.into_iter().collect::<Vec<_>>();
                borsh::to_vec(&array)
            },
            Tensor::Int16(array) => {
                if array.ndim() > 1 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "cannot flatten a multidimensional array",
                    ));
                }
                let array = array.into_iter().collect::<Vec<_>>();
                borsh::to_vec(&array)
            },
            Tensor::Int32(array) => {
                if array.ndim() > 1 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "cannot flatten a multidimensional array",
                    ));
                }
                let array = array.into_iter().collect::<Vec<_>>();
                borsh::to_vec(&array)
            },
            Tensor::Int64(array) => {
                if array.ndim() > 1 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "cannot flatten a multidimensional array",
                    ));
                }
                let array = array.into_iter().collect::<Vec<_>>();
                borsh::to_vec(&array)
            },
            Tensor::Uint8(array) => {
                if array.ndim() > 1 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "cannot flatten a multidimensional array",
                    ));
                }
                let array = array.into_iter().collect::<Vec<_>>();
                borsh::to_vec(&array)
            },
            Tensor::Uint16(array) => {
                if array.ndim() > 1 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "cannot flatten a multidimensional array",
                    ));
                }
                let array = array.into_iter().collect::<Vec<_>>();
                borsh::to_vec(&array)
            },
            Tensor::Uint32(array) => {
                if array.ndim() > 1 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "cannot flatten a multidimensional array",
                    ));
                }
                let array = array.into_iter().collect::<Vec<_>>();
                borsh::to_vec(&array)
            },
            Tensor::Uint64(array) => {
                if array.ndim() > 1 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "cannot flatten a multidimensional array",
                    ));
                }
                let array = array.into_iter().collect::<Vec<_>>();
                borsh::to_vec(&array)
            },
            Tensor::Uint8D1(array) => {
                if array.ndim() > 1 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "cannot flatten a multidimensional array",
                    ));
                }
                let array = array.into_iter().collect::<Vec<_>>();
                borsh::to_vec(&array)
            },
            Tensor::Float32(array) => {
                if array.ndim() > 1 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "cannot flatten a multidimensional array",
                    ));
                }
                let array = array.into_iter().collect::<Vec<_>>();
                borsh::to_vec(&array)
            },
            Tensor::Float64(array) => {
                if array.ndim() > 1 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "cannot flatten a multidimensional array",
                    ));
                }
                let array = array.into_iter().collect::<Vec<_>>();
                borsh::to_vec(&array)
            },
        }
    }
}

impl TryFrom<Tensor> for Value {
    type Error = std::io::Error;

    fn try_from(value: Tensor) -> Result<Self, Self::Error> {
        match value {
            Tensor::Int8(array) => array
                .try_into()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e)),
            Tensor::Int16(array) => array
                .try_into()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e)),
            Tensor::Int32(array) => array
                .try_into()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e)),
            Tensor::Int64(array) => array
                .try_into()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e)),
            Tensor::Uint8(array) => array
                .try_into()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e)),
            Tensor::Uint16(array) => array
                .try_into()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e)),
            Tensor::Uint32(array) => array
                .try_into()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e)),
            Tensor::Uint64(array) => array
                .try_into()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e)),
            Tensor::Uint8D1(array) => array
                .try_into()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e)),
            Tensor::Float32(array) => array
                .try_into()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e)),
            Tensor::Float64(array) => array
                .try_into()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e)),
        }
    }
}

// #[derive(BorshSerialize, BorshDeserialize)]
// pub enum Vector {
//     Int8(Vec<i8>),
//     Int16(Vec<i16>),
//     Int32(Vec<i32>),
//     Int64(Vec<i64>),
//     Uint8(Vec<u8>),
//     Uint16(Vec<u16>),
//     Uint32(Vec<u32>),
//     Uint64(Vec<u64>),
//     Float32(Vec<f32>),
//     Float64(Vec<f64>),
// }
//
// impl TryFrom<Tensor> for Vector {
//     type Error = std::io::Error;
//
//     fn try_from(value: Tensor) -> Result<Self, Self::Error> {
//         let vector = match value {
//             Tensor::Int8(array) => {
//                 if array.ndim() > 1 {
//                     return Err(std::io::Error::new(
//                         std::io::ErrorKind::InvalidData,
//                         "array has more than one dimension",
//                     ));
//                 }
//                 Vector::Int8(array.as_standard_layout().into_iter().collect::<Vec<_>>())
//             },
//             Tensor::Int16(array) => {
//                 if array.ndim() > 1 {
//                     return Err(std::io::Error::new(
//                         std::io::ErrorKind::InvalidData,
//                         "array has more than one dimension",
//                     ));
//                 }
//                 Vector::Int16(array.as_standard_layout().into_iter().collect::<Vec<_>>())
//             },
//             Tensor::Int32(array) => {
//                 if array.ndim() > 1 {
//                     return Err(std::io::Error::new(
//                         std::io::ErrorKind::InvalidData,
//                         "array has more than one dimension",
//                     ));
//                 }
//                 Vector::Int32(array.as_standard_layout().into_iter().collect::<Vec<_>>())
//             },
//             Tensor::Int64(array) => {
//                 if array.ndim() > 1 {
//                     return Err(std::io::Error::new(
//                         std::io::ErrorKind::InvalidData,
//                         "array has more than one dimension",
//                     ));
//                 }
//                 Vector::Int64(array.as_standard_layout().into_iter().collect::<Vec<_>>())
//             },
//             Tensor::Uint8(array) => {
//                 if array.ndim() > 1 {
//                     return Err(std::io::Error::new(
//                         std::io::ErrorKind::InvalidData,
//                         "array has more than one dimension",
//                     ));
//                 }
//                 Vector::Uint8(array.as_standard_layout().into_iter().collect::<Vec<_>>())
//             },
//             Tensor::Uint16(array) => {
//                 if array.ndim() > 1 {
//                     return Err(std::io::Error::new(
//                         std::io::ErrorKind::InvalidData,
//                         "array has more than one dimension",
//                     ));
//                 }
//                 Vector::Uint16(array.as_standard_layout().into_iter().collect::<Vec<_>>())
//             },
//             Tensor::Uint32(array) => {
//                 if array.ndim() > 1 {
//                     return Err(std::io::Error::new(
//                         std::io::ErrorKind::InvalidData,
//                         "array has more than one dimension",
//                     ));
//                 }
//                 Vector::Uint32(array.as_standard_layout().into_iter().collect::<Vec<_>>())
//             },
//             Tensor::Uint64(array) => {
//                 if array.ndim() > 1 {
//                     return Err(std::io::Error::new(
//                         std::io::ErrorKind::InvalidData,
//                         "array has more than one dimension",
//                     ));
//                 }
//                 Vector::Uint64(array.as_standard_layout().into_iter().collect::<Vec<_>>())
//             },
//             Tensor::Uint8D1(array) => {
//                 if array.ndim() > 1 {
//                     return Err(std::io::Error::new(
//                         std::io::ErrorKind::InvalidData,
//                         "array has more than one dimension",
//                     ));
//                 }
//                 Vector::Uint8(array.as_standard_layout().into_iter().collect::<Vec<_>>())
//             },
//             Tensor::Float32(array) => {
//                 if array.ndim() > 1 {
//                     return Err(std::io::Error::new(
//                         std::io::ErrorKind::InvalidData,
//                         "array has more than one dimension",
//                     ));
//                 }
//                 Vector::Float32(array.as_standard_layout().into_iter().collect::<Vec<_>>())
//             },
//             Tensor::Float64(array) => {
//                 if array.ndim() > 1 {
//                     return Err(std::io::Error::new(
//                         std::io::ErrorKind::InvalidData,
//                         "array has more than one dimension",
//                     ));
//                 }
//                 Vector::Float64(array.as_standard_layout().into_iter().collect::<Vec<_>>())
//             },
//         };
//
//         Ok(vector)
//     }
// }
