use ndarray::ArrayD;

pub fn deserialize_u8(shape: &[usize], data: &[u8]) -> anyhow::Result<ArrayD<u8>> {
    ArrayD::<u8>::from_shape_vec(shape, data.to_vec()).map_err(Into::into)
}

pub fn deserialize_i8(shape: &[usize], data: &[u8]) -> anyhow::Result<ArrayD<i8>> {
    let view_data = data.iter().map(|byte| *byte as i8).collect::<Vec<_>>();
    ArrayD::<i8>::from_shape_vec(shape, view_data).map_err(Into::into)
}

pub fn deserialize_i16(shape: &[usize], data: &[u8]) -> anyhow::Result<ArrayD<i16>> {
    let view_data = data
        .chunks_exact(2)
        .map(TryInto::try_into)
        .map(Result::unwrap)
        .map(i16::from_le_bytes)
        .collect::<Vec<_>>();
    ArrayD::<i16>::from_shape_vec(shape, view_data).map_err(Into::into)
}

pub fn deserialize_u16(shape: &[usize], data: &[u8]) -> anyhow::Result<ArrayD<u16>> {
    let view_data = data
        .chunks_exact(2)
        .map(TryInto::try_into)
        .map(Result::unwrap)
        .map(u16::from_le_bytes)
        .collect::<Vec<_>>();
    ArrayD::<u16>::from_shape_vec(shape, view_data).map_err(Into::into)
}

pub fn deserialize_i32(shape: &[usize], data: &[u8]) -> anyhow::Result<ArrayD<i32>> {
    let view_data = data
        .chunks_exact(4)
        .map(TryInto::try_into)
        .map(Result::unwrap)
        .map(i32::from_le_bytes)
        .collect::<Vec<_>>();
    ArrayD::<i32>::from_shape_vec(shape, view_data).map_err(Into::into)
}

pub fn deserialize_u32(shape: &[usize], data: &[u8]) -> anyhow::Result<ArrayD<u32>> {
    let view_data = data
        .chunks_exact(4)
        .map(TryInto::try_into)
        .map(Result::unwrap)
        .map(u32::from_le_bytes)
        .collect::<Vec<_>>();
    ArrayD::<u32>::from_shape_vec(shape, view_data).map_err(Into::into)
}

pub fn deserialize_i64(shape: &[usize], data: &[u8]) -> anyhow::Result<ArrayD<i64>> {
    let view_data = data
        .chunks_exact(8)
        .map(TryInto::try_into)
        .map(Result::unwrap)
        .map(i64::from_le_bytes)
        .collect::<Vec<_>>();
    ArrayD::<i64>::from_shape_vec(shape, view_data).map_err(Into::into)
}

pub fn deserialize_u64(shape: &[usize], data: &[u8]) -> anyhow::Result<ArrayD<u64>> {
    let view_data = data
        .chunks_exact(8)
        .map(TryInto::try_into)
        .map(Result::unwrap)
        .map(u64::from_le_bytes)
        .collect::<Vec<_>>();
    ArrayD::<u64>::from_shape_vec(shape, view_data).map_err(Into::into)
}

pub fn deserialize_f32(shape: &[usize], data: &[u8]) -> anyhow::Result<ArrayD<f32>> {
    let view_data = data
        .chunks_exact(4)
        .map(TryInto::try_into)
        .map(Result::unwrap)
        .map(f32::from_le_bytes)
        .collect::<Vec<_>>();
    ArrayD::<f32>::from_shape_vec(shape, view_data).map_err(Into::into)
}

pub fn deserialize_f64(shape: &[usize], data: &[u8]) -> anyhow::Result<ArrayD<f64>> {
    let view_data = data
        .chunks_exact(8)
        .map(TryInto::try_into)
        .map(Result::unwrap)
        .map(f64::from_le_bytes)
        .collect::<Vec<_>>();
    ArrayD::<f64>::from_shape_vec(shape, view_data).map_err(Into::into)
}
