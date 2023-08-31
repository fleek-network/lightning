use dashmap::DashMap;
use lightning_interfaces::types::ServiceId;
use lightning_interfaces::ServiceHandleInterface;
use triomphe::Arc;

use crate::handle::ServiceHandle;

#[derive(Clone, Default)]
pub struct ServiceCollection {
    services: Arc<DashMap<ServiceId, ServiceHandle>>,
}

impl ServiceCollection {
    #[inline(always)]
    pub fn get_handle(&self, id: ServiceId) -> Option<ServiceHandle> {
        self.services.get(&id).as_deref().cloned()
    }

    pub fn insert(&self, handle: ServiceHandle) {
        if self
            .services
            .insert(handle.get_service_id(), handle)
            .is_some()
        {
            panic!("service id already in use.");
        }
    }
}
