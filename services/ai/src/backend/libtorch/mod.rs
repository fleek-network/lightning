pub mod inference;
mod mnist;
pub mod train;

use crate::opts;

impl From<opts::Device> for tch::Device {
    fn from(value: opts::Device) -> Self {
        match value {
            opts::Device::Cpu => tch::Device::Cpu,
            opts::Device::Cuda(index) => tch::Device::Cuda(index),
        }
    }
}
