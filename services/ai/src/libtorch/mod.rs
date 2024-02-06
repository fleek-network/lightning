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

pub fn load_and_run_model(
    model: Bytes,
    input: Bytes,
    device: opts::Device,
) -> anyhow::Result<String> {
    let module = CModule::load_data_on_device(&mut Cursor::new(model), device.into())?;
    let input = imagenet::load_image_and_resize224_from_memory(input.as_ref())?;
    let output = module.forward_t(&input.unsqueeze(0), false);

    // Argument seems to be the max line width, docs are not clear about this.
    output.to_string(500).map_err(Into::into)
}

impl From<opts::Device> for tch::Device {
    fn from(value: opts::Device) -> Self {
        match value {
            opts::Device::Cpu => tch::Device::Cpu,
            opts::Device::Cuda(index) => tch::Device::Cuda(index),
        }
    }
}
