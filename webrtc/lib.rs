extern crate boxfnonce;
extern crate log;
extern crate servo_media_streams;
use servo_media_streams::MediaStreamType;
use servo_media_streams::registry::MediaStreamId;

use std::fmt::Display;
use std::str::FromStr;

use boxfnonce::SendBoxFnOnce;

pub mod thread;

pub use thread::WebRtcController;

#[derive(Debug)]
pub enum WebrtcError {
    Backend(String),
}

pub type WebrtcResult = Result<(), WebrtcError>;

impl<T: Display> From<T> for WebrtcError {
    fn from(x: T) -> Self {
        WebrtcError::Backend(x.to_string())
    }
}

/// This trait is implemented by backends and should never be used directly by
/// the client. Use WebRtcController instead
pub trait WebRtcControllerBackend: Send {
    fn configure(&mut self, stun_server: &str, policy: BundlePolicy) -> WebrtcResult;
    fn set_remote_description(
        &mut self,
        SessionDescription,
        cb: SendBoxFnOnce<'static, ()>,
    ) -> WebrtcResult;
    fn set_local_description(
        &mut self,
        SessionDescription,
        cb: SendBoxFnOnce<'static, ()>,
    ) -> WebrtcResult;
    fn add_ice_candidate(&mut self, candidate: IceCandidate) -> WebrtcResult;
    fn create_offer(&mut self, cb: SendBoxFnOnce<'static, (SessionDescription,)>) -> WebrtcResult;
    fn create_answer(&mut self, cb: SendBoxFnOnce<'static, (SessionDescription,)>) -> WebrtcResult;
    fn add_stream(&mut self, stream: &MediaStreamId) -> WebrtcResult;
    fn internal_event(&mut self, event: thread::InternalEvent) -> WebrtcResult;
    fn quit(&mut self);
}

pub trait WebRtcSignaller: Send {
    fn on_ice_candidate(&self, controller: &WebRtcController, candidate: IceCandidate);
    fn on_negotiation_needed(&self, controller: &WebRtcController);
    fn close(&self);
    fn on_add_stream(&self, stream: &MediaStreamId, ty: MediaStreamType);

    fn update_signaling_state(&self, _: SignalingState) {}
    fn update_gathering_state(&self, _: GatheringState) {}
    fn update_ice_connection_state(&self, _: IceConnectionState) {}
}

pub trait WebRtcBackend {
    type Controller: WebRtcControllerBackend + 'static;

    fn construct_webrtc_controller(
        signaller: Box<dyn WebRtcSignaller>,
        thread: WebRtcController,
    ) -> Self::Controller;
}

/// https://www.w3.org/TR/webrtc/#rtcsdptype
#[derive(Copy, Clone, Hash, Debug, PartialEq, Eq)]
pub enum SdpType {
    Answer,
    Offer,
    Pranswer,
    Rollback,
}

#[derive(Copy, Clone, Hash, Debug, PartialEq, Eq)]
pub enum DescriptionType {
    Local,
    Remote,
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
            _ => return Err(()),
        })
    }
}

/// https://www.w3.org/TR/webrtc/#rtcsessiondescription-class
///
/// https://developer.mozilla.org/en-US/docs/Web/API/RTCSessionDescription
#[derive(Clone, Hash, Debug, PartialEq, Eq)]
pub struct SessionDescription {
    pub type_: SdpType,
    pub sdp: String,
}

/// https://www.w3.org/TR/webrtc/#rtcicecandidate-interface
///
/// https://developer.mozilla.org/en-US/docs/Web/API/RTCIceCandidate
#[derive(Clone, Hash, Debug, PartialEq, Eq)]
pub struct IceCandidate {
    pub sdp_mline_index: u32,
    pub candidate: String,
    // XXXManishearth this is missing a bunch
}

/// https://developer.mozilla.org/en-US/docs/Web/API/RTCPeerConnection#RTCBundlePolicy_enum
#[derive(Clone, Copy, Hash, Debug, PartialEq, Eq)]
pub enum BundlePolicy {
    Balanced,
    MaxCompat,
    MaxBundle,
}

impl BundlePolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            BundlePolicy::Balanced => "balanced",
            BundlePolicy::MaxCompat => "max-compat",
            BundlePolicy::MaxBundle => "max-bundle",
        }
    }
}

/// https://www.w3.org/TR/webrtc/#rtcsignalingstate-enum
#[derive(Clone, Copy, Hash, Debug, PartialEq, Eq)]
pub enum SignalingState {
    Stable,
    HaveLocalOffer,
    HaveRemoteOffer,
    HaveLocalPranswer,
    HaveRemotePranswer,
    Closed,
}

/// https://www.w3.org/TR/webrtc/#rtcicegatheringstate-enum
#[derive(Clone, Copy, Hash, Debug, PartialEq, Eq)]
pub enum GatheringState {
    New,
    Gathering,
    Complete,
}

/// https://www.w3.org/TR/webrtc/#rtciceconnectionstate-enum
#[derive(Clone, Copy, Hash, Debug, PartialEq, Eq)]
pub enum IceConnectionState {
    New,
    Checking,
    Connected,
    Completed,
    Disconnected,
    Failed,
    Closed,
}
