use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum ClientMessage {
    #[serde(rename = "start_sending")]
    StartSending,
    #[serde(rename = "start_receiving")]
    StartReceiving { stream_id: Option<String> },
    #[serde(rename = "webrtc_offer")]
    WebRtcOffer { offer: SdpMessage },
    #[serde(rename = "webrtc_answer")]
    WebRtcAnswer { answer: SdpMessage },
    #[serde(rename = "ice_candidate")]
    IceCandidate { candidate: CandidateMessage },
    #[serde(rename = "get_available_streams")]
    GetAvailableStreams,
    #[serde(rename = "stop_stream")]
    StopStream,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum ServerMessage {
    #[serde(rename = "sender_ready")]
    SenderReady { connection_id: String },
    #[serde(rename = "available_streams")]
    AvailableStreams { streams: Vec<String> },
    #[serde(rename = "stream_available")]
    StreamAvailable { stream_id: String },
    #[serde(rename = "stream_ended")]
    StreamEnded { stream_id: String },
    #[serde(rename = "webrtc_offer")]
    WebRtcOffer { offer: SdpMessage },
    #[serde(rename = "webrtc_answer")]
    WebRtcAnswer { answer: SdpMessage },
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SdpMessage {
    pub sdp: String,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CandidateMessage {
    pub candidate: String,
    #[serde(rename = "sdpMid")]
    pub sdp_mid: Option<String>,
    #[serde(rename = "sdpMLineIndex")]
    pub sdp_mline_index: Option<u16>,
}
