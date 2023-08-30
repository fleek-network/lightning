use fn_sdk::internal::{OnConnectedArgs, OnDisconnectedArgs, OnMessageArgs};
use lightning_interfaces::ServiceHandleInterface;
use triomphe::Arc;

use crate::service::Service;

#[derive(Clone)]
pub struct ServiceHandle {
    inner: Arc<Service>,
}

impl ServiceHandleInterface for ServiceHandle {
    fn message(&self, args: OnMessageArgs) {
        (self.inner.message)(args);
    }

    fn connected(&self, args: OnConnectedArgs) {
        (self.inner.connected)(args);
    }

    fn disconnected(&self, args: OnDisconnectedArgs) {
        (self.inner.disconnected)(args);
    }
}
