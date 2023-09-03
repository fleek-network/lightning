use std::time::{SystemTime, UNIX_EPOCH};

#[inline(always)]
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// Generate an access token from connection id and randomizer.
#[inline(always)]
pub fn to_access_token(connection_id: u64, randomizer: &[u8; 40]) -> [u8; 48] {
    let mut value = [0; 48];
    value[0..8].copy_from_slice(connection_id.to_be_bytes().as_slice());
    value[8..].copy_from_slice(randomizer);
    value
}

/// Split an access token to connection id and randomizer.
#[inline(always)]
pub fn split_access_token(token: &[u8; 48]) -> (u64, &[u8; 40]) {
    let connection_id_be = arrayref::array_ref!(token, 0, 8);
    let connection_id = u64::from_be_bytes(*connection_id_be);
    let randomizer = arrayref::array_ref!(token, 8, 40);
    (connection_id, randomizer)
}
