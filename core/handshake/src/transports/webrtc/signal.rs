use std::net::{IpAddr, SocketAddr};

use anyhow::Result;
use axum::extract::{ConnectInfo, State};
use axum::routing::post;
use axum::{Json, Router};
use str0m::change::{SdpAnswer, SdpOffer};
use str0m::{Candidate, Rtc};
use tokio::sync::mpsc::{Receiver, Sender};
use tower_http::cors::CorsLayer;
use triomphe::Arc;

use super::driver::{Connection, ConnectionMap};
use crate::schema::{HandshakeRequestFrame, RequestFrame};

struct SignalState {
    // Map of rtc connection states
    client_map: ConnectionMap,
    // Sender for incoming connections (given to each connection)
    conn_tx: Sender<(HandshakeRequestFrame, IpAddr, Receiver<RequestFrame>)>,
    // Our address candidate used for the sdp response
    host: Candidate,
}

pub fn router(
    udp_addr: SocketAddr,
    client_map: ConnectionMap,
    conn_tx: Sender<(HandshakeRequestFrame, IpAddr, Receiver<RequestFrame>)>,
) -> Result<Router> {
    Ok(Router::new()
        .route("/sdp", post(handler))
        .layer(CorsLayer::permissive())
        .with_state(Arc::new(SignalState {
            client_map,
            conn_tx,
            host: Candidate::host(udp_addr)?,
        })))
}

async fn handler(
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<SignalState>>,
    Json(offer): Json<SdpOffer>,
) -> Result<Json<SdpAnswer>, String> {
    let mut rtc = Rtc::new();
    rtc.add_local_candidate(state.host.clone());
    let answer = rtc
        .sdp_api()
        .accept_offer(offer)
        .map_err(|e| e.to_string())?;

    let ip = peer_addr.ip();
    state
        .client_map
        .insert(ip, Connection::new(rtc, ip, state.conn_tx.clone()));

    Ok(Json(answer))
}
