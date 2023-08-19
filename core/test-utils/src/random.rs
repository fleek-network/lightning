use rand::rngs::StdRng;
use rand::SeedableRng;

pub fn get_seedable_rng() -> StdRng {
    let seed: [u8; 32] = (0..32).collect::<Vec<u8>>().try_into().unwrap();
    SeedableRng::from_seed(seed)
}
