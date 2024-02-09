#![allow(dead_code)]

use bytes::Bytes;

pub fn load_and_run_model(
    _model: Bytes,
    _input: Bytes,
    _device: crate::Device,
) -> anyhow::Result<Bytes> {
    todo!()
}
