use anyhow::Result;
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

type WorkerSocket = affair::Socket<RTCSessionDescription, Result<RTCSessionDescription>>;

pub(crate) fn router(socket: WorkerSocket) -> Router {
    Router::new()
        .route("/sdp", post(handler))
        .with_state(socket)
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
