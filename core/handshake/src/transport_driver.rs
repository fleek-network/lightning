use lightning_interfaces::ServiceHandleProviderInterface;
use tokio::task::JoinHandle;

use crate::state::StateRef;
use crate::transports::{StaticSender, Transport, TransportReceiver};

/// Attach the provided transport to the state.
pub fn attach_transport_to_state<P: ServiceHandleProviderInterface, T: Transport>(
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
                res.handle,
                res.connection_id,
                receiver,
            ));
        }
    })
}

#[inline(always)]
async fn run_receive_loop<P: ServiceHandleProviderInterface, T: Transport>(
    _state: StateRef<P>,
    _handle: P::Handle,
    _connection_id: u64,
    mut receiver: T::Receiver,
) {
    while let Some(_frame) = receiver.recv().await {}
}
