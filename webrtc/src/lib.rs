extern crate boxfnonce;

use std::any::Any;
use std::str::FromStr;

use boxfnonce::SendBoxFnOnce;

pub mod thread;

pub trait MediaStream: Any + Send {
    fn as_any(&self) -> &Any;
}

pub use thread::WebRtcController;

/// This trait is implemented by backends and should never be used directly by
/// the client. Use WebRtcController instead
pub trait WebRtcControllerBackend: Send + Sync {
    fn configure(&self, stun_server: &str, policy: BundlePolicy);
    /// Invariant: Callback must not reentrantly invoke any methods on the controller
    fn set_remote_description(&self, SessionDescription, cb: SendBoxFnOnce<'static, ()>);
    /// Invariant: Callback must not reentrantly invoke any methods on the controller
    fn set_local_description(&self, SessionDescription, cb: SendBoxFnOnce<'static, ()>);
    fn add_ice_candidate(&self, candidate: IceCandidate);
    fn create_offer(&self, cb: SendBoxFnOnce<'static, (SessionDescription,)>);
    fn create_answer(&self, cb: SendBoxFnOnce<'static, (SessionDescription,)>);
    fn add_stream(&self, stream: &MediaStream);
}

pub trait WebRtcSignaller: Send {
    fn on_ice_candidate(&self, controller: &WebRtcController, candidate: IceCandidate);
    /// Invariant: Must not reentrantly invoke any methods on the controller
    fn on_negotiation_needed(&self, controller: &WebRtcController);
    fn close(&self, reason: String);
}

pub trait WebRtcBackend {
    type Controller: WebRtcControllerBackend + 'static;

    fn construct_webrtc_controller(
        signaller: Box<WebRtcSignaller>,
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
