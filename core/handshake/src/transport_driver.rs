use lightning_interfaces::ExecutorProviderInterface;
use tokio::task::JoinHandle;

use crate::state::{StateRef, TransportPermission};
use crate::transports::{StaticSender, Transport, TransportReceiver};

/// Attach the provided transport to the state.
pub fn attach_transport_to_state<P: ExecutorProviderInterface, T: Transport>(
    state: StateRef<P>,
    mut transport: T,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some((frame, sender, receiver)) = transport.accept().await {
            let sender: StaticSender = sender.into();

            let Some(res) = state.process_handshake(frame, sender) else {
                continue;
            };

            tokio::spawn(run_receive_loop::<P, T>(
                state.clone(),
                res.connection_id,
                res.perm,
                receiver,
            ));
        }
    })
}

#[inline(always)]
async fn run_receive_loop<P: ExecutorProviderInterface, T: Transport>(
    state: StateRef<P>,
    connection_id: u64,
    perm: TransportPermission,
    mut receiver: T::Receiver,
) {
    while let Some(frame) = receiver.recv().await {
        state.on_request_frame(perm, connection_id, frame);
    }

    state.on_transport_closed(perm, connection_id);
}
