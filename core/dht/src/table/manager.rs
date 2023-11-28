use anyhow::Result;
use lightning_interfaces::types::NodeIndex;

use crate::table::server::TableKey;

pub enum Event {
    Pong { index: NodeIndex, timestamp: u64 },
    Unresponsive { index: NodeIndex },
}

pub trait Manager {
    fn closest_contacts(&self, key: TableKey) -> Vec<NodeIndex>;

    fn handle_event(&mut self, event: Event);
}
