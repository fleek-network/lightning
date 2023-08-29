use crate::infu_collection::Collection;
use crate::types::ServiceId;
use crate::{ConfigConsumer, ConfigProviderInterface, WithStartAndShutdown};

pub struct Service {
    pub on_start: Box<dyn Fn()>,
    pub on_open: Box<dyn Fn()>,
    pub on_close: Box<dyn Fn()>,
    pub on_message: Box<dyn Fn()>,
}

#[infusion::service]
pub trait ServiceExecutorInterface<C: Collection>:
    WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync
{
    fn _init(config: ::ConfigProviderInterface) {
        Self::init(config.get::<Self>())
    }

    fn init(config: Self::Config) -> anyhow::Result<Self>;

    fn register_service(&self, service_id: ServiceId, service: Service);
}
