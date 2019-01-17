use std::str::FromStr;

pub trait WebRtcController: Send {
    fn notify_signal_server_error(&self);
    fn set_remote_description(&self, SessionDescription);
    fn set_local_description(&self, SessionDescription);
    fn add_ice_candidate(&self, candidate: IceCandidate);
    fn trigger_negotiation(&self);
}

pub trait WebRtcSignaller: Send {
    fn send_sdp_offer(&self, offer: String);
    fn on_ice_candidate(&self, candidate: IceCandidate);
    fn close(&self, reason: String);
}

pub trait WebRtcBackend {
    type Controller: WebRtcController;

    fn start_webrtc_controller(signaller: Box<WebRtcSignaller>) -> Self::Controller;
}

/// https://www.w3.org/TR/webrtc/#rtcsdptype
pub enum SdpType {
    Answer,
    Offer,
    Pranswer,
    Rollback,
}

impl SdpType {
    pub fn as_str(self) -> &'static str {
        match self {
            SdpType::Answer => "answer",
            SdpType::Offer => "offer",
            SdpType::Pranswer => "pranswer",
            SdpType::Rollback => "rollback",
        }
    }
}

impl FromStr for SdpType {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, ()> {
        Ok(match s {
            "answer" => SdpType::Answer,
            "offer" => SdpType::Offer,
            "pranswer" => SdpType::Pranswer,
            "rollback" => SdpType::Rollback,
            _ => return Err(())
        })
    }
}

/// https://www.w3.org/TR/webrtc/#rtcsessiondescription-class
///
/// https://developer.mozilla.org/en-US/docs/Web/API/RTCSessionDescription
pub struct SessionDescription {
    pub type_: SdpType,
    pub sdp: String,
}

/// https://www.w3.org/TR/webrtc/#rtcicecandidate-interface
///
/// https://developer.mozilla.org/en-US/docs/Web/API/RTCIceCandidate
pub struct IceCandidate {
    pub sdp_mline_index: u32,
    pub candidate: String,
    // XXXManishearth this is missing a bunch
}
