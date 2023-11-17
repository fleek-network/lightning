use std::error::Error;

pub struct Registry {}

pub trait Handler {
    type Target;

    fn init(registry: Registry) -> Result<Self::Target, Box<dyn Error>>;

    fn post_init(target: &mut Self::Target, registry: Registry) {}
}
