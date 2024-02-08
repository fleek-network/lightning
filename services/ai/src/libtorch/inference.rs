use std::io::Cursor;

use bytes::Bytes;
use tch::{CModule, Tensor};

// Todo: optionally, a user could ask use to load weights for the model.
/// Loads model on the device and returns output of model given the input.
pub fn load_and_run_model(
    model: Bytes,
    input: Bytes,
    device: crate::Device,
) -> anyhow::Result<Bytes> {
    if !tch::Cuda::is_available() && device.is_cuda() {
        anyhow::bail!("device {device:?} is not available in this system");
    }

    // Load model on device.
    let module = CModule::load_data_on_device(&mut Cursor::new(model), device.into())?;

    // Once https://github.com/pytorch/pytorch/issues/48525
    // is resolved, we can do some validation on the input.

    // Deserialize tensor.
    let input = Tensor::load_from_stream(Cursor::new(input))?;

    // This means data has to be preprocessed by client to make sure
    // it is a valid input for the model.
    let output = module.forward_ts(&[input])?;

    // Serialize the result.
    // Todo: calculate capacity upfront.
    let mut buffer = Vec::new();
    output.save_to_stream(Cursor::new(&mut buffer))?;

    Ok(buffer.into())
}
