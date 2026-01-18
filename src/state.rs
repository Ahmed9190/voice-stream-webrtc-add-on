use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use dashmap::DashMap;
use webrtc::track::track_remote::TrackRemote;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::peer_connection::RTCPeerConnection;

#[derive(Clone)]
pub struct AppState {
    pub streams: Arc<DashMap<String, Stream>>,
    pub connections: Arc<DashMap<String, Connection>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            streams: Arc::new(DashMap::new()),
            connections: Arc::new(DashMap::new()),
        }
    }
}

pub struct Stream {
    pub stream_id: String,
    pub source_track: Arc<TrackRemote>,
    pub receivers: Arc<RwLock<Vec<Arc<TrackLocalStaticRTP>>>>,
    pub notify_shutdown: broadcast::Sender<()>,
}

pub struct Connection {
    pub connection_id: String,
    pub pc: Arc<RTCPeerConnection>,
    pub role: ConnectionRole,
    pub stream_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionRole {
    Sender,
    Receiver,
}
