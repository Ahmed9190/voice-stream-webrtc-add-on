use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use webrtc::api::APIBuilder;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::TrackLocalWriter;
use webrtc::track::track_remote::TrackRemote;
use tracing::{info, error, debug};

pub async fn create_peer_connection() -> Result<Arc<RTCPeerConnection>> {
    // Create a MediaEngine object to configure the supported codecs
    let mut m = MediaEngine::default();
    m.register_default_codecs()?;

    let mut registry = Registry::new();
    registry = register_default_interceptors(registry, &mut m)?;

    let api = APIBuilder::new()
        .with_media_engine(m)
        .with_interceptor_registry(registry)
        .build();

    let config = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec![], // LAN only
            ..Default::default()
        }],
        ..Default::default()
    };

    let pc = api.new_peer_connection(config).await?;

    Ok(Arc::new(pc))
}

pub async fn start_relay_loop(
    source_track: Arc<TrackRemote>,
    receivers: Arc<RwLock<Vec<Arc<TrackLocalStaticRTP>>>>,
    mut shutdown: broadcast::Receiver<()>,
) {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown.recv() => {
                    info!("Relay loop stopped for track {}", source_track.id());
                    break;
                }
                result = source_track.read_rtp() => {
                    match result {
                        Ok((packet, _attributes)) => {
                            let receivers_guard = receivers.read().await;
                            for receiver in receivers_guard.iter() {
                                if let Err(e) = receiver.write_rtp(&packet).await {
                                    debug!("Failed to write to receiver track: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Error reading from source track: {}", e);
                            break;
                        }
                    }
                }
            }
        }
    });
}