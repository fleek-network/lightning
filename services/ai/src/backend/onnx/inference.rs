#![allow(dead_code)]

use bytes::Bytes;
use ort::Session;

use crate::backend::onnx::exec::Executor;
use crate::tensor::numpy;

pub fn load_and_run_model(
    model: Bytes,
    input: Bytes,
    _device: crate::Device,
) -> anyhow::Result<Bytes> {
    // Todo: map to execution provider from device.
    // Build model.
    let session = Session::builder()?.with_model_from_memory(model.as_ref())?;
    let executor = Executor::new(session);

    // Deserialize input.
    let input = numpy::load_tensor_from_mem(input.as_ref())?;

    // Run inference on input.
    let outputs = executor.run(input)?;

    // Encode and return output.
    serde_json::to_string(&outputs)
        .map(Into::into)
        .map_err(Into::into)
}
