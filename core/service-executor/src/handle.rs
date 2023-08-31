use fn_sdk::internal::{OnConnectedArgs, OnDisconnectedArgs, OnMessageArgs};
use lightning_interfaces::ServiceHandleInterface;
use triomphe::Arc;

use crate::service::Service;

#[derive(Clone)]
pub struct ServiceHandle {
    inner: Arc<Service>,
}

impl ServiceHandleInterface for ServiceHandle {
    #[inline(always)]
    fn get_service_id(&self) -> u32 {
        self.inner.id
    }

    #[inline(always)]
    fn message(&self, args: OnMessageArgs) {
        (self.inner.message)(args);
    }

    #[inline(always)]
    fn connected(&self, args: OnConnectedArgs) {
        (self.inner.connected)(args);
    }

    #[inline(always)]
    fn disconnected(&self, args: OnDisconnectedArgs) {
        (self.inner.disconnected)(args);
    }
}

impl From<Service> for ServiceHandle {
    fn from(value: Service) -> Self {
        Self {
            inner: Arc::new(value),
        }
    }
}

#[macro_export]
macro_rules! handle {
    ($id:expr, $module:tt) => {
        $crate::handle::ServiceHandle::from($crate::service::Service {
            id: $id,
            start: $module::on_start,
            connected: $module::on_connected,
            disconnected: $module::on_disconnected,
            message: $module::on_message,
            respond: $module::on_event_response,
        })
    };
}
