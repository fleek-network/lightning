use crate::table::TableKey;

pub fn leading_zero_bits(key_a: &TableKey, key_b: &TableKey) -> usize {
    let distance = key_a
        .iter()
        .zip(key_b.iter())
        .map(|(a, b)| a ^ b)
        .collect::<Vec<_>>();
    let mut index = 0;
    for byte in distance {
        let leading_zeros = byte.leading_zeros();
        index += leading_zeros;
        if leading_zeros < 8 {
            break;
        }
    }
    index as usize
}
