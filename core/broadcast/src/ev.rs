
use infusion::c;
use lightning_interfaces::infu_collection::Collection;

use lightning_interfaces::{ListenerConnector, ListenerInterface, ApplicationInterface, PoolSender, SenderInterface};

use crate::db::Database;
use crate::frame::Frame;
use crate::interner::Interner;
use crate::peers::{Peers};

struct State<C: Collection> {
    db: Database,
    interner: Interner,
    peers: Peers<PoolSender<C, c![C::ConnectionPoolInterface], Frame>>
}

pub async fn main_loop<C: Collection>(
    db: Database,
    _sqr: c![C::ApplicationInterface::SyncExecutor],
    _topology: c![C::TopologyInterface],
    (mut listener, _connector): ListenerConnector<C, c![C::ConnectionPoolInterface], Frame>,
) {
    let _state = State::<C> {
        db,
        interner: Interner::new(1024),
        peers: Peers::default()
    };

    let (sender, _) = listener.accept().await.unwrap();
    let _pk = sender.pk();

}

