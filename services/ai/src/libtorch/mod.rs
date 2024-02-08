mod mnist;
pub mod train;

use std::io::Cursor;

use bytes::Bytes;
use tch::nn::ModuleT;
use tch::vision::imagenet;
use tch::{CModule, Tensor};

use crate::opts;

const RESNET18_WEIGHTS_FILE_PATH: &str = "weights/resnet18.ot";
const RESNET34_WEIGHTS_FILE_PATH: &str = "weights/resnet34.ot";
pub const IMAGENET_CLASS_COUNT: i64 = imagenet::CLASS_COUNT;

// Todo: optionally, a user could ask use to load weights for the model.
/// Loads model on the device and returns output of model given the input.
pub fn load_and_run_model(
    model: Bytes,
    input: Bytes,
    device: opts::Device,
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

// Todo: remove these.
pub fn run_resnet18(
    image: &[u8],
    device: opts::Device,
    class_count: i64,
) -> anyhow::Result<Vec<(f64, String)>> {
    // Init store.
    let mut vs = tch::nn::VarStore::new(device.into());

    // Create the model.
    let net = tch::vision::resnet::resnet18(&vs.root(), class_count);

    // Load weights.
    vs.load(RESNET18_WEIGHTS_FILE_PATH)?;

    // Load the image file and resize it to the usual imagenet dimension of 224x224.
    let image = imagenet::load_image_and_resize224_from_memory(image)?;

    // Apply the forward pass of the model.
    let output = net
        .forward_t(&image.unsqueeze(0), false)
        .softmax(-1, tch::Kind::Float);

    // Return the top 5 categories.
    Ok(imagenet::top(&output, 5))
}

pub fn run_resnet34(
    image: &[u8],
    device: opts::Device,
    class_count: i64,
) -> anyhow::Result<Vec<(f64, String)>> {
    // Init store.
    let mut vs = tch::nn::VarStore::new(device.into());

    // Create the model.
    let net = tch::vision::resnet::resnet34(&vs.root(), class_count);

    // Load weights.
    vs.load(RESNET34_WEIGHTS_FILE_PATH)?;

    // Load the image file and resize it to the usual imagenet dimension of 224x224.
    let image = imagenet::load_image_and_resize224_from_memory(image)?;

    // Apply the forward pass of the model.
    let output = net
        .forward_t(&image.unsqueeze(0), false)
        .softmax(-1, tch::Kind::Float);

    // Return the top 5 categories.
    Ok(imagenet::top(&output, 5))
}

impl From<opts::Device> for tch::Device {
    fn from(value: opts::Device) -> Self {
        match value {
            opts::Device::Cpu => tch::Device::Cpu,
            opts::Device::Cuda(index) => tch::Device::Cuda(index),
        }
    }
}
