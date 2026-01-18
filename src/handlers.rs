use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{error, info};
use uuid::Uuid;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::TrackLocal;

use crate::state::{AppState, Connection, ConnectionRole, Stream as AppStream};
use crate::messages::{ClientMessage, ServerMessage, SdpMessage};
use crate::webrtc_manager;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let connection_id = Uuid::new_v4().to_string();
    
    // Create a channel to send messages from other tasks (like WebRTC callbacks) to this WS
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<ServerMessage>();

    // Spawn a task to write to WebSocket
    let write_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg) {
                if sender.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        }
    });

    // Send initial available streams
    let stream_ids: Vec<String> = state.streams.iter().map(|s| s.key().clone()).collect();
    let _ = tx.send(ServerMessage::AvailableStreams { streams: stream_ids });

    // Handle incoming messages
    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Text(text) = msg {
            if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                match client_msg {
                    ClientMessage::StartSending => {
                        handle_start_sending(&state, &connection_id, &tx).await;
                    }
                    ClientMessage::StartReceiving { stream_id } => {
                        handle_start_receiving(&state, &connection_id, stream_id, &tx).await;
                    }
                    ClientMessage::WebRtcOffer { offer } => {
                        handle_offer(&state, &connection_id, offer, &tx).await;
                    }
                    ClientMessage::WebRtcAnswer { answer } => {
                        handle_answer(&state, &connection_id, answer).await;
                    }
                    ClientMessage::GetAvailableStreams => {
                         let streams: Vec<String> = state.streams.iter().map(|s| s.key().clone()).collect();
                         let _ = tx.send(ServerMessage::AvailableStreams { streams });
                    }
                    ClientMessage::StopStream => {
                        cleanup_connection(&state, &connection_id).await;
                    }
                    _ => {} // Ignore others for now
                }
            }
        }
    }

    // Cleanup
    cleanup_connection(&state, &connection_id).await;
    write_task.abort();
}

async fn handle_start_sending(state: &AppState, connection_id: &str, tx: &tokio::sync::mpsc::UnboundedSender<ServerMessage>) {
    let pc = match webrtc_manager::create_peer_connection().await {
        Ok(pc) => pc,
        Err(e) => {
            error!("Failed to create PC: {}", e);
            let _ = tx.send(ServerMessage::Error { message: "Failed to create PeerConnection".into() });
            return;
        }
    };

    let connection_id_clone = connection_id.to_string();
    let state_clone = state.clone();

    pc.on_track(Box::new(move |track, _, _| {
        let connection_id = connection_id_clone.clone();
        let state = state_clone.clone();
        
        Box::pin(async move {
            info!("Track received from sender {}", connection_id);
            let stream_id = format!("stream_{}", connection_id);
            let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

            let receivers = Arc::new(RwLock::new(Vec::new()));
            
            // Create the stream entry
            let stream = AppStream {
                stream_id: stream_id.clone(),
                source_track: track.clone(),
                receivers: receivers.clone(),
                notify_shutdown: shutdown_tx.clone(),
            };
            
            state.streams.insert(stream_id.clone(), stream);

            // Start relay
            webrtc_manager::start_relay_loop(track, receivers, shutdown_rx).await;
        })
    }));
    
    // Register connection
    state.connections.insert(connection_id.to_string(), Connection {
        connection_id: connection_id.to_string(),
        pc: pc.clone(),
        role: ConnectionRole::Sender,
        stream_id: None,
    });

    let _ = tx.send(ServerMessage::SenderReady { connection_id: connection_id.to_string() });
}

async fn handle_start_receiving(state: &AppState, connection_id: &str, stream_id_req: Option<String>, tx: &tokio::sync::mpsc::UnboundedSender<ServerMessage>) {
    let target_stream_id = match stream_id_req {
        Some(id) => id,
        None => {
            match state.streams.iter().last() {
                Some(r) => r.key().clone(),
                None => {
                     let _ = tx.send(ServerMessage::Error { message: "No audio stream available".into() });
                     return;
                }
            }
        }
    };

    let stream_ref = match state.streams.get(&target_stream_id) {
        Some(s) => s,
        None => {
            let _ = tx.send(ServerMessage::Error { message: "Stream ended or not found".into() });
            return;
        }
    };

    let pc = match webrtc_manager::create_peer_connection().await {
        Ok(pc) => pc,
        Err(e) => {
            error!("Failed to create PC: {}", e);
            return;
        }
    };

    let local_track = Arc::new(TrackLocalStaticRTP::new(
        webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability {
            mime_type: "audio/opus".to_owned(),
            ..Default::default()
        },
        "audio".to_owned(),
        "webrtc_server".to_owned(),
    ));

    if let Err(e) = pc.add_track(Arc::clone(&local_track) as Arc<dyn TrackLocal + Send + Sync>).await {
         error!("Failed to add track to PC: {}", e);
         return;
    }

    stream_ref.receivers.write().await.push(local_track);

    state.connections.insert(connection_id.to_string(), Connection {
        connection_id: connection_id.to_string(),
        pc: pc.clone(),
        role: ConnectionRole::Receiver,
        stream_id: Some(target_stream_id),
    });

    let offer = match pc.create_offer(None).await {
        Ok(o) => o,
        Err(e) => {
            error!("Failed to create offer: {}", e);
            return;
        }
    };

    if let Err(e) = pc.set_local_description(offer.clone()).await {
        error!("Failed to set local desc: {}", e);
        return;
    }

    // Wait for ICE gathering
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let sdp = match pc.local_description().await {
        Some(d) => d.sdp,
        None => return,
    };

    let _ = tx.send(ServerMessage::WebRtcOffer {
        offer: SdpMessage { sdp, kind: "offer".into() }
    });
}

async fn handle_offer(state: &AppState, connection_id: &str, offer: SdpMessage, tx: &tokio::sync::mpsc::UnboundedSender<ServerMessage>) {
    if let Some(conn) = state.connections.get(connection_id) {
        let pc = conn.pc.clone();
        
        let desc = match RTCSessionDescription::offer(offer.sdp) {
            Ok(d) => d,
            Err(e) => { error!("Invalid SDP: {}", e); return; }
        };

        if let Err(e) = pc.set_remote_description(desc).await {
             error!("Failed to set remote desc: {}", e);
             return;
        }

        let answer = match pc.create_answer(None).await {
            Ok(a) => a,
            Err(e) => { error!("Create answer failed: {}", e); return; }
        };

        if let Err(e) = pc.set_local_description(answer).await {
             error!("Set local desc failed: {}", e);
             return;
        }
        
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        let sdp = match pc.local_description().await {
            Some(d) => d.sdp,
            None => return,
        };

        let _ = tx.send(ServerMessage::WebRtcAnswer {
            answer: SdpMessage { sdp, kind: "answer".into() }
        });
    }
}

async fn handle_answer(state: &AppState, connection_id: &str, answer: SdpMessage) {
    if let Some(conn) = state.connections.get(connection_id) {
        let pc = conn.pc.clone();
        let desc = match RTCSessionDescription::answer(answer.sdp) {
            Ok(d) => d,
            Err(e) => { error!("Invalid SDP answer: {}", e); return; }
        };
        if let Err(e) = pc.set_remote_description(desc).await {
             error!("Failed to set remote answer: {}", e);
        }
    }
}

async fn cleanup_connection(state: &AppState, connection_id: &str) {
    if let Some((_, conn)) = state.connections.remove(connection_id) {
        info!("Cleaning up connection {}", connection_id);
        let _ = conn.pc.close().await;

        if conn.role == ConnectionRole::Sender {
             let stream_id = format!("stream_{}", connection_id);
             if let Some((_, stream)) = state.streams.remove(&stream_id) {
                 let _ = stream.notify_shutdown.send(());
             }
        }
    }
}