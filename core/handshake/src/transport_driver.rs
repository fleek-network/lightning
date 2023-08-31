use std::ops::ControlFlow;

use lightning_interfaces::ExecutorProviderInterface;
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

use crate::state::{StateRef, TransportPermission};
use crate::transports::{self, Transport, TransportReceiver};

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum TransportConfig {
    WebRTC(transports::webrtc::WebRtcConfig),
}

pub async fn attach_transport_by_config<P: ExecutorProviderInterface>(
    state: StateRef<P>,
    config: TransportConfig,
) -> anyhow::Result<JoinHandle<()>> {
    let transport = match config {
        TransportConfig::WebRTC(config) => {
            transports::webrtc::WebRtcTransport::bind(config).await?
        },
    };

    Ok(attach_transport_to_state(state, transport))
}

pub fn attach_transport_to_state<P: ExecutorProviderInterface, T: Transport>(
    state: StateRef<P>,
    transport: T,
) -> JoinHandle<()> {
    tokio::spawn(async {
        state
            .shutdown
            .clone()
            .fold_until_shutdown((transport, state), handle_connection)
            .await;
    })
}

#[inline]
async fn handle_connection<P: ExecutorProviderInterface, T: Transport>(
    (mut transport, state): (T, StateRef<P>),
) -> ControlFlow<(), (T, StateRef<P>)> {
    let Some((frame, sender, receiver)) = transport.accept().await else {
        return ControlFlow::Break(());
    };

    let Some(res) = state.process_handshake(frame, sender.into()) else {
        return ControlFlow::Continue((transport, state));
    };

    tokio::spawn(run_receive_loop::<P, T>(
        state.clone(),
        res.connection_id,
        res.perm,
        receiver,
    ));

    ControlFlow::Continue((transport, state))
}

#[inline(always)]
async fn run_receive_loop<P: ExecutorProviderInterface, T: Transport>(
    state: StateRef<P>,
    connection_id: u64,
    perm: TransportPermission,
    receiver: T::Receiver,
) {
    state
        .shutdown
        .fold_until_shutdown(receiver, |mut receiver| async {
            let Some(frame) = receiver.recv().await else { return ControlFlow::Break(()); };
            state.on_request_frame(perm, connection_id, frame);
            ControlFlow::Continue(receiver)
        })
        .await;

    state.on_transport_closed(perm, connection_id);
}
