use dashmap::DashMap;
use lightning_interfaces::types::ServiceId;
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
}
