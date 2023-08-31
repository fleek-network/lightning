use anyhow::Result;
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

use crate::shutdown::ShutdownWaiter;

type WorkerSocket = affair::Socket<RTCSessionDescription, Result<RTCSessionDescription>>;

pub async fn start_signaling_server(
    waiter: ShutdownWaiter,
    config: super::WebRtcConfig,
    socket: WorkerSocket,
) -> Result<()> {
    let app = Router::new()
        .route("/sdp", post(handler))
        .with_state(socket);

    axum::Server::bind(&config.signal_address)
        .serve(app.into_make_service())
        .with_graceful_shutdown(waiter.wait_for_shutdown())
        .await?;

    Ok(())
}

async fn handler(
    State(socket): State<WorkerSocket>,
    Json(req): Json<RTCSessionDescription>,
) -> Result<Json<RTCSessionDescription>, String> {
    socket
        .run(req)
        .await
        .map_err(|e| format!("internal error: {e}"))?
        .map_err(|e| format!("{e}"))
        .map(Json)
}
