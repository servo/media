extern crate boxfnonce;
extern crate servo_media_streams;
use servo_media_streams::MediaStream;

use std::str::FromStr;

use boxfnonce::SendBoxFnOnce;

pub mod thread;

pub use thread::WebRtcController;

/// This trait is implemented by backends and should never be used directly by
/// the client. Use WebRtcController instead
pub trait WebRtcControllerBackend: Send {
    fn configure(&mut self, stun_server: &str, policy: BundlePolicy);
    fn set_remote_description(&mut self, SessionDescription, cb: SendBoxFnOnce<'static, ()>);
    fn set_local_description(&mut self, SessionDescription, cb: SendBoxFnOnce<'static, ()>);
    fn add_ice_candidate(&mut self, candidate: IceCandidate);
    fn create_offer(&mut self, cb: SendBoxFnOnce<'static, (SessionDescription,)>);
    fn create_answer(&mut self, cb: SendBoxFnOnce<'static, (SessionDescription,)>);
    fn add_stream(&mut self, stream: &mut MediaStream);
    fn internal_event(&mut self, event: thread::InternalEvent);
    fn quit(&mut self);
}

pub trait WebRtcSignaller: Send {
    fn on_ice_candidate(&self, controller: &WebRtcController, candidate: IceCandidate);
    fn on_negotiation_needed(&self, controller: &WebRtcController);
    fn close(&self);
    fn on_add_stream(&self, stream: Box<MediaStream>);
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
