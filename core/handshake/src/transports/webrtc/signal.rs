use std::net::{IpAddr, SocketAddr};

use anyhow::Result;
use axum::extract::{ConnectInfo, State};
use axum::routing::post;
use axum::{Extension, Json, Router};
use lightning_interfaces::ExecutorProviderInterface;
use str0m::change::{SdpAnswer, SdpOffer};
use str0m::{Candidate, Rtc};
use tokio::sync::mpsc::{Receiver, Sender};
use tower_http::cors::CorsLayer;
use triomphe::Arc;

use super::driver::{Connection, ConnectionMap};
use crate::handshake::Context;
use crate::schema::{HandshakeRequestFrame, RequestFrame};

struct SignalState {
    // Map of rtc connection states
    client_map: ConnectionMap,
    // Sender for incoming connections (given to each connection)
    conn_tx: Sender<(HandshakeRequestFrame, IpAddr, Receiver<RequestFrame>)>,
    // Our address candidate used for the sdp response
    host: Vec<Candidate>,
}

pub fn router<P: ExecutorProviderInterface>(
    udp_addrs: Vec<SocketAddr>,
    client_map: ConnectionMap,
    conn_tx: Sender<(HandshakeRequestFrame, IpAddr, Receiver<RequestFrame>)>,
) -> Result<Router> {
    Ok(Router::new()
        .route("/sdp", post(handler::<P>))
        .layer(CorsLayer::permissive())
        .with_state(Arc::new(SignalState {
            client_map,
            conn_tx,
            host: udp_addrs
                .into_iter()
                .map(|s| {
                    Candidate::host(s, "udp")
                        .expect("failed to parse candidate from socket address")
                })
                .collect(),
        })))
}

async fn handler<P: ExecutorProviderInterface>(
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<SignalState>>,
    Extension(_provider): Extension<Context<P>>,
    Json(offer): Json<SdpOffer>,
) -> Result<Json<SdpAnswer>, String> {
    let mut rtc = Rtc::new();
    for host in &state.host {
        rtc.add_local_candidate(host.clone());
    }
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
