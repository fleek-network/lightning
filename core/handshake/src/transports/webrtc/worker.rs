use std::sync::Arc;

use affair::AsyncWorker;
use anyhow::anyhow;
use async_trait::async_trait;
use bytes::Bytes;
use log::{debug, error};
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

use crate::schema::HandshakeRequestFrame;

pub(crate) struct IncomingConnectionWorker {
    pub conn_tx: tokio::sync::mpsc::Sender<(HandshakeRequestFrame, Arc<RTCDataChannel>)>,
}

#[async_trait]
impl AsyncWorker for IncomingConnectionWorker {
    type Request = RTCSessionDescription;
    type Response = anyhow::Result<RTCSessionDescription>;

    async fn handle(&mut self, req: Self::Request) -> Self::Response {
        // Setup media engine to configure available codecs.
        let mut m = MediaEngine::default();
        m.register_default_codecs()?;

        // Create a InterceptorRegistry. This is the user configurable RTP/RTCP Pipeline.
        // This provides NACKs, RTCP Reports and other features.
        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut m)?;
        let api = APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .build();

        // TODO: We should be able to skip stun and ICE gathering completely since nodes already
        // know their public address. Figure out a way to provide it here.
        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_owned()],
                ..Default::default()
            }],
            ..Default::default()
        };
        let peer_connection = Arc::new(api.new_peer_connection(config).await?);

        let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);
        peer_connection.on_peer_connection_state_change(Box::new(
            move |s: RTCPeerConnectionState| {
                if s == RTCPeerConnectionState::Failed {
                    // Wait until PeerConnection has had no network activity for 30 seconds or
                    // another failure. It may be reconnected using an ICE Restart.
                    debug!("WebRTC connection failed");
                    let _ = done_tx.try_send(());
                } else {
                    debug!("webrtc connection state has changed: {s}");
                }
                Box::pin(async {})
            },
        ));

        // Register data channel creation handling
        let conn_tx = self.conn_tx.clone();
        peer_connection.on_data_channel(Box::new(move |d: Arc<RTCDataChannel>| {
            let d_label = d.label().to_owned();
            let d_id = d.id();
            debug!("New data channel {d_label} {d_id}");
            let conn_tx = conn_tx.clone();
            Box::pin(async move {
                let d2 = d.clone();
                let d_label2 = d_label.clone();
                d.on_close(Box::new(move || {
                    debug!("Data channel closed: {d_label} {d_id}");
                    Box::pin(async {})
                }));
                d.on_open(Box::new(move || {
                    debug!("Data channel opened: {d_label2} {d_id}");
                    Box::pin(async move {})
                }));
                d.on_message(Box::new(move |msg: DataChannelMessage| {
                    let conn_tx = conn_tx.clone();
                    let d2 = d2.clone();
                    if msg.is_string {
                        error!("received string but expected binary data");
                        return Box::pin(async {});
                    }
                    // receive a single handshake frame and send it
                    match HandshakeRequestFrame::decode(&msg.data) {
                        Ok(frame) => Box::pin(async move {
                            if let Err(e) = conn_tx.send((frame, d2)).await {
                                error!("failed to send incoming connection to transport: {e}");
                            }
                        }),
                        Err(e) => {
                            error!("failed to decode handshake request frame: {e}");
                            Box::pin(async {})
                        },
                    }
                }));
            })
        }));

        // Set the remote SessionDescription
        peer_connection.set_remote_description(req).await?;

        // Create an answer
        let answer = peer_connection.create_answer(None).await?;

        // Create channel that is blocked until ICE Gathering is complete
        let mut gather_complete = peer_connection.gathering_complete_promise().await;

        // Sets the LocalDescription, and starts our UDP listeners
        peer_connection.set_local_description(answer).await?;

        // Block until ICE Gathering is complete, disabling trickle ICE
        // we do this because we only can exchange one signaling message
        let _ = gather_complete.recv().await;

        let response = peer_connection
            .local_description()
            .await
            .ok_or(anyhow!("failed to get local description"))?;

        tokio::spawn(async move {
            done_rx.recv().await;
            peer_connection.close().await.ok();
        });

        // return our local description
        Ok(response)
    }
}

// TODO: Use or create a webrtc library that doesn't require silly things
pub struct SendWorker(pub Arc<RTCDataChannel>);

#[async_trait]
impl AsyncWorker for SendWorker {
    type Request = Bytes;
    type Response = ();

    async fn handle(&mut self, req: Self::Request) -> Self::Response {
        if let Err(e) = self.0.send(&req).await {
            error!("Failed to send payload: {e}");
        }
    }
}
