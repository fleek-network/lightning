pub trait Transport {
    fn init() -> Self;
}

pub struct WT;

impl Transport for WT {
    fn init() -> Self {
        todo!()
    }
}
