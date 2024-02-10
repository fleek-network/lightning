#![allow(dead_code)]

use std::io::Cursor;

use bytes::Bytes;
use ort::{inputs, Session};

use crate::backend::onnx::exec::Executor;
use crate::tensor::{numpy, Tensor};

pub fn load_and_run_model(
    model: Bytes,
    input: Bytes,
    _device: crate::Device,
) -> anyhow::Result<Bytes> {
    // Todo: map to execution provider from device.
    // Build model.
    let session = Session::builder()?.with_model_from_memory(model.as_ref())?;
    let executor = Executor::new(session);

    // Deserialize tensor.
    let input = numpy::load_tensor_from_mem(input.as_ref())?;

    // Run inference on input.
    let output = executor.run(input)?;

    // Encode output in npy file.
    let mut buffer = Vec::new();
    numpy::write_tensor(Cursor::new(&mut buffer), output)?;

    Ok(buffer.into())
}
