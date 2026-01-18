use axum::{
    extract::State,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde_json::json;
use crate::state::AppState;
use std::net::SocketAddr;
use std::path::PathBuf;
use tracing::{info, error};
use axum_server::tls_rustls::RustlsConfig;

pub async fn start_audio_server(state: AppState, use_ssl: bool) {
    let app = Router::new()
        .route("/stream/status", get(status_handler))
        .route("/stream/latest.mp3", get(latest_stream_handler))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8081));

    if use_ssl {
        let cert_path = PathBuf::from("ssl/cert.pem");
        let key_path = PathBuf::from("ssl/key.pem");

        match RustlsConfig::from_pem_file(cert_path, key_path).await {
            Ok(config) => {
                info!("Audio Stream Server listening on {} (HTTPS)", addr);
                if let Err(e) = axum_server::bind_rustls(addr, config)
                    .serve(app.into_make_service())
                    .await 
                {
                    error!("Audio server error: {}", e);
                }
            },
            Err(e) => {
                error!("Failed to load SSL for audio server: {}", e);
            }
        }
    } else {
        info!("Audio Stream Server listening on {} (HTTP)", addr);
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    }
}

async fn status_handler(State(state): State<AppState>) -> impl IntoResponse {
    let streams: Vec<String> = state.streams.iter().map(|s| s.key().clone()).collect();
    Json(json!({ "active_streams": streams }))
}

async fn latest_stream_handler(State(_state): State<AppState>) -> impl IntoResponse {
    // In a full implementation, this would:
    // 1. Find the latest stream.
    // 2. Create a new TrackLocalStaticRTP.
    // 3. Add it to the stream's receivers.
    // 4. Decode RTP (Opus) -> PCM -> Encode MP3.
    // 5. Stream bytes.
    
    // For this Rust rebuild without external C-libs for MP3 encoding, 
    // we return a 501 Not Implemented with a message.
    (
        axum::http::StatusCode::NOT_IMPLEMENTED,
        "MP3 Streaming requires ffmpeg/libmp3lame bindings which are not included in this pure Rust prototype. Use WebRTC (port 8080) for audio."
    )
}