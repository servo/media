pub trait WebRtcController: Send {
    fn notify_signal_server_error(&self);
    fn notify_sdp(&self, type_: String, sdp: String);
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
