mod state;
mod handlers;
mod webrtc_manager;
mod messages;
mod audio_server;

use axum::{
    routing::get,
    Router,
    response::Json,
    extract::State,
};
use serde_json::json;
use std::net::SocketAddr;
use std::path::PathBuf;
use tracing::{info, warn};
use crate::state::AppState;
use axum_server::tls_rustls::RustlsConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("Starting WebRTC Voice Streaming Backend (Rust)");

    let state = AppState::new();

    // Check for SSL certificates
    let cert_path = PathBuf::from("ssl/cert.pem");
    let key_path = PathBuf::from("ssl/key.pem");
    let use_ssl = cert_path.exists() && key_path.exists();

    if use_ssl {
        info!("SSL certificates found. Starting in SECURE mode (WSS/HTTPS).");
    } else {
        warn!("SSL certificates not found (checked ssl/cert.pem, ssl/key.pem). Starting in INSECURE mode (WS/HTTP).");
    }

    // Start Audio Server (HTTP/HTTPS MP3) on port 8081 in a separate task
    let audio_state = state.clone();
    let audio_use_ssl = use_ssl;
    tokio::spawn(async move {
        audio_server::start_audio_server(audio_state, audio_use_ssl).await;
    });

    // Start Signaling Server (WebSocket + HTTP) on port 8080
    let app = Router::new()
        .route("/ws", get(handlers::ws_handler))
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .with_state(state.clone());

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    
    if use_ssl {
        let config = RustlsConfig::from_pem_file(
            cert_path,
            key_path,
        ).await?;

        info!("Signaling Server listening on {} (WSS)", addr);
        axum_server::bind_rustls(addr, config)
            .serve(app.into_make_service())
            .await?;
    } else {
        info!("Signaling Server listening on {} (WS)", addr);
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
    }

    Ok(())
}

async fn health_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "webrtc_available": true,
        "audio_server_running": true, // Assumed since task started
        "active_streams": state.streams.len(),
        "connected_clients": state.connections.len(),
    }))
}

async fn metrics_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({
        "active_connections": state.connections.len(),
        "active_streams": state.streams.len(),
        "webrtc_available": true,
    }))
}
