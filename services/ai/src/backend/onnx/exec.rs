use std::ops::Deref;

use ort::{inputs, Session, TensorElementType, ValueType};

use crate::tensor::Tensor;

pub struct Executor {
    session: Session,
}

impl Executor {
    pub fn new(session: Session) -> Self {
        Self { session }
    }

    /// Runs model on the input.
    pub fn run(&self, input: Tensor) -> anyhow::Result<Tensor> {
        let tensor = match input {
            Tensor::Int32(array) => {
                let outputs = self.session.run(inputs![array.view()]?)?;
                outputs["output"]
                    .extract_tensor::<i32>()?
                    .view()
                    .deref()
                    .to_owned()
                    .into()
            },
            Tensor::Int64(array) => {
                let outputs = self.session.run(inputs![array.view()]?)?;
                outputs["output"]
                    .extract_tensor::<i64>()?
                    .view()
                    .deref()
                    .to_owned()
                    .into()
            },
            Tensor::Uint32(array) => {
                let outputs = self.session.run(inputs![array.view()]?)?;
                outputs["output"]
                    .extract_tensor::<u32>()?
                    .view()
                    .deref()
                    .to_owned()
                    .into()
            },
            Tensor::Uint64(array) => {
                let outputs = self.session.run(inputs![array.view()]?)?;
                outputs["output"]
                    .extract_tensor::<u64>()?
                    .view()
                    .deref()
                    .to_owned()
                    .into()
            },
            Tensor::Float32(array) => {
                let outputs = self.session.run(inputs![array.view()]?)?;
                outputs["output"]
                    .extract_tensor::<f32>()?
                    .view()
                    .deref()
                    .to_owned()
                    .into()
            },
            Tensor::Float64(array) => {
                let outputs = self.session.run(inputs![array.view()]?)?;
                outputs["output"]
                    .extract_tensor::<f64>()?
                    .view()
                    .deref()
                    .to_owned()
                    .into()
            },
        };

        Ok(tensor)
    }
}
