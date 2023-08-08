use lightning_interfaces::ServiceScope;
use lightning_schema::AutoImplSerde;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub enum ScopedFrame {
    Hello {
        scope: ServiceScope,
    },
    Message {
        scope: ServiceScope,
        payload: Vec<u8>,
    },
}

impl AutoImplSerde for ScopedFrame {}
