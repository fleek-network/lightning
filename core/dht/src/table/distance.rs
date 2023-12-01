use crate::table::worker::TableKey;

pub type Distance = TableKey;

pub fn leading_zero_bits(key_a: &TableKey, key_b: &TableKey) -> usize {
    let distance = distance(key_a, key_b);
    let mut index = 0;
    for byte in distance {
        let leading_zeros = byte.leading_zeros();
        index += leading_zeros;
        if byte > 0 {
            break;
        }
    }
    index as usize
}

pub fn distance(key_a: &TableKey, key_b: &TableKey) -> Distance {
    key_a
        .iter()
        .zip(key_b.iter())
        .map(|(a, b)| a ^ b)
        .collect::<Vec<_>>()
        .try_into()
        .expect("Converting to array to succeed")
}
