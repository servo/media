#![feature(fn_traits)]

extern crate boxfnonce;
extern crate log;
extern crate servo_media_streams;
extern crate uuid;

use servo_media_streams::registry::MediaStreamId;
use servo_media_streams::MediaStreamType;

use std::fmt::Display;
use std::str::FromStr;
use std::sync::mpsc::Sender;

use boxfnonce::SendBoxFnOnce;
use uuid::Uuid;

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
    fn create_data_channel(
        &mut self,
        init: &WebRtcDataChannelInit,
        channel: Sender<Box<dyn WebRtcDataChannelBackend>>
    ) -> WebrtcResult;
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

    fn on_data_channel(&self, _: Box<dyn WebRtcDataChannelBackend>) {}
}

pub struct WebRtcDataChannelCallbacks {
    pub open: Option<SendBoxFnOnce<'static, ()>>,
    pub error: Option<SendBoxFnOnce<'static, (WebrtcError,)>>,
    pub message: Option<Box<dyn Fn(String) + Send + 'static>>,
    pub close: Option<SendBoxFnOnce<'static, ()>>,
}

impl WebRtcDataChannelCallbacks {
    pub fn new() -> WebRtcDataChannelCallbacks {
        WebRtcDataChannelCallbacks {
            open: None,
            error: None,
            message: None,
            close: None,
        }
    }

    pub fn open(&mut self) {
        if let Some(callback) = self.open.take() {
            callback.call();
        };
    }

    pub fn error(&mut self, error: WebrtcError) {
        if let Some(callback) = self.error.take() {
            callback.call(error);
        };
    }

    pub fn message(&self, message: String) {
        if let Some(ref callback) = self.message {
            callback.call((message,));
        };
    }

    pub fn close(&mut self) {
        if let Some(callback) = self.close.take() {
            callback.call();
        };
    }
}

pub trait WebRtcDataChannelBackend: Send {
    fn set_on_open(&self, Box<dyn FnOnce() + Send + 'static>);
    fn set_on_error(&self, Box<dyn FnOnce(WebrtcError,) + Send + 'static>);
    fn set_on_message(&self, Box<dyn Fn(String) + Send + 'static>);
    fn set_on_close(&self, Box<dyn FnOnce() + Send + 'static>);
    fn send(&self, &str) -> WebrtcResult;
    fn close(&self);
}

pub trait InnerWebRtcDataChannel: Send {
    fn send(&self, &str) -> WebrtcResult;
}

// https://www.w3.org/TR/webrtc/#dom-rtcdatachannelinit
// plus `label`
pub struct WebRtcDataChannelInit {
    pub label: String,
    pub ordered: bool,
    pub max_packet_life_time: Option<u16>,
    pub max_retransmits: Option<u16>,
    pub protocol: String,
    pub negotiated: bool,
    pub id: Option<u16>,
}

impl Default for WebRtcDataChannelInit {
    fn default() -> WebRtcDataChannelInit {
        WebRtcDataChannelInit {
            label: Uuid::new_v4().to_string(),
            ordered: true,
            max_packet_life_time: None,
            max_retransmits: None,
            protocol: String::new(),
            negotiated: false,
            id: None,
        }
    }
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
