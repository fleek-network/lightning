use std::collections::HashMap;

use bytes::Bytes;
use serde::{Deserialize, Serialize};

pub mod imagenet;
pub mod service_api;

pub fn to_array_d<T>(data: Vec<T>, shape: Vec<u64>, order: npyz::Order) -> ndarray::ArrayD<T> {
    use ndarray::ShapeBuilder;

    let shape = shape.into_iter().map(|x| x as usize).collect::<Vec<_>>();
    let true_shape = shape.set_f(order == npyz::Order::Fortran);

    ndarray::ArrayD::from_shape_vec(true_shape, data)
        .unwrap_or_else(|e| panic!("shape error: {}", e))
}

#[derive(Deserialize, Serialize)]
pub struct Output {
    pub format: String,
    pub outputs: HashMap<String, String>,
}

#[derive(Deserialize, Serialize)]
pub enum Input {
    Raw(Bytes),
    Map(HashMap<String, Bytes>),
}
