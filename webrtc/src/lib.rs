use std::str::FromStr;

pub trait WebRtcController: Send {
    fn notify_signal_server_error(&self);
    fn set_remote_description(&self, RTCSessionDescription);
    fn notify_ice(&self, sdp_mline_index: u32, candidate: String);
    fn trigger_negotiation(&self);
}

pub trait WebRtcSignaller: Send {
    fn send_sdp_offer(&self, offer: String);
    fn send_ice_candidate(&self, mlineindex: u32, candidate: String);
    fn close(&self, reason: String);
}

pub trait WebRtcBackend {
    type Controller: WebRtcController;

    fn start_webrtc_controller(signaller: Box<WebRtcSignaller>) -> Self::Controller;
}

pub enum RTCSdpType {
    Answer,
    Offer,
    Pranswer,
    Rollback,
}

impl RTCSdpType {
    pub fn as_str(self) -> &'static str {
        match self {
            RTCSdpType::Answer => "answer",
            RTCSdpType::Offer => "offer",
            RTCSdpType::Pranswer => "pranswer",
            RTCSdpType::Rollback => "rollback",
        }
    }
}

impl FromStr for RTCSdpType {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, ()> {
        Ok(match s {
            "answer" => RTCSdpType::Answer,
            "offer" => RTCSdpType::Offer,
            "pranswer" => RTCSdpType::Pranswer,
            "rollback" => RTCSdpType::Rollback,
            _ => return Err(())
        })
    }
}

pub struct RTCSessionDescription {
    pub type_: RTCSdpType,
    pub sdp: String,
}
