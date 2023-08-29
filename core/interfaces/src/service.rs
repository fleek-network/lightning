use fn_sdk::internal::{
    OnConnectedArgs,
    OnDisconnectedArgs,
    OnEventResponseArgs,
    OnMessageArgs,
    OnStartArgs,
};

use crate::infu_collection::Collection;
use crate::types::ServiceId;
use crate::{ConfigConsumer, ConfigProviderInterface, WithStartAndShutdown};

pub struct Service {
    pub start: fn(OnStartArgs),
    pub connected: fn(OnConnectedArgs),
    pub disconnected: fn(OnDisconnectedArgs),
    pub message: fn(OnMessageArgs),
    pub respond: fn(OnEventResponseArgs),
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
