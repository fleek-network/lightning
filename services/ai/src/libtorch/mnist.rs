//! Utils for reading MNIST hand-written digit dataset from memory.
use std::io;
use std::io::{Cursor, Read};

use bytes::Bytes;
use tch::{Kind, Tensor};

use crate::libtorch::train::Dataset;

fn read_u32<T: Read>(reader: &mut T) -> io::Result<u32> {
    let mut b = vec![0u8; 4];
    reader.read_exact(&mut b)?;
    let (result, _) = b.iter().rev().fold((0u64, 1u64), |(s, basis), &x| {
        (s + basis * u64::from(x), basis * 256)
    });
    Ok(result as u32)
}

fn check_magic_number<T: Read>(reader: &mut T, expected: u32) -> io::Result<()> {
    let magic_number = read_u32(reader)?;
    if magic_number != expected {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("incorrect magic number {magic_number} != {expected}"),
        ));
    }
    Ok(())
}

fn read_labels(data: Bytes) -> io::Result<Tensor> {
    let mut buf_reader = Cursor::new(data);
    check_magic_number(&mut buf_reader, 2049)?;
    let samples = read_u32(&mut buf_reader)?;
    let mut data = vec![0u8; samples as usize];
    buf_reader.read_exact(&mut data)?;
    Ok(Tensor::from_slice(&data).to_kind(Kind::Int64))
}

fn read_images(data: Bytes) -> io::Result<Tensor> {
    let mut buf_reader = Cursor::new(data);
    check_magic_number(&mut buf_reader, 2051)?;
    let samples = read_u32(&mut buf_reader)?;
    let rows = read_u32(&mut buf_reader)?;
    let cols = read_u32(&mut buf_reader)?;
    let data_len = samples * rows * cols;
    let mut data = vec![0u8; data_len as usize];
    buf_reader.read_exact(&mut data)?;
    let tensor = Tensor::from_slice(&data)
        .view((i64::from(samples), i64::from(rows * cols)))
        .to_kind(Kind::Float);
    Ok(tensor / 255.)
}

pub fn load_from_mem(dataset: Dataset) -> io::Result<tch::vision::dataset::Dataset> {
    let train_images = read_images(dataset.train_images)?;
    let train_labels = read_labels(dataset.train_labels)?;
    let test_images = read_images(dataset.validation_images)?;
    let test_labels = read_labels(dataset.validation_labels)?;
    Ok(tch::vision::dataset::Dataset {
        train_images,
        train_labels,
        test_images,
        test_labels,
        labels: 10,
    })
}
