use std::sync::mpsc::{channel, Sender};
use std::thread;

use log::error;

use boxfnonce::SendBoxFnOnce;

use crate::{
    BundlePolicy, DescriptionType, IceCandidate, MediaStreamId, SdpType, SessionDescription,
};
use crate::{WebRtcBackend, WebRtcControllerBackend, WebRtcSignaller};

#[derive(Clone)]
/// Entry point for all client webrtc interactions.
pub struct WebRtcController {
    sender: Sender<RtcThreadEvent>,
}

impl WebRtcController {
    pub fn new<T: WebRtcBackend>(signaller: Box<WebRtcSignaller>) -> Self {
        let (sender, receiver) = channel();

        let t = WebRtcController { sender };

        let mut controller = T::construct_webrtc_controller(signaller, t.clone());

        thread::spawn(move || {
            while let Ok(event) = receiver.recv() {
                if !handle_rtc_event(&mut controller, event) {
                    // shut down event loop
                    break;
                }
            }
        });

        t
    }
    pub fn configure(&self, stun_server: String, policy: BundlePolicy) {
        let _ = self
            .sender
            .send(RtcThreadEvent::ConfigureStun(stun_server, policy));
    }
    pub fn set_remote_description(&self, desc: SessionDescription, cb: SendBoxFnOnce<'static, ()>) {
        let _ = self
            .sender
            .send(RtcThreadEvent::SetRemoteDescription(desc, cb));
    }
    pub fn set_local_description(&self, desc: SessionDescription, cb: SendBoxFnOnce<'static, ()>) {
        let _ = self
            .sender
            .send(RtcThreadEvent::SetLocalDescription(desc, cb));
    }
    pub fn add_ice_candidate(&self, candidate: IceCandidate) {
        let _ = self.sender.send(RtcThreadEvent::AddIceCandidate(candidate));
    }
    pub fn create_offer(&self, cb: SendBoxFnOnce<'static, (SessionDescription,)>) {
        let _ = self.sender.send(RtcThreadEvent::CreateOffer(cb));
    }
    pub fn create_answer(&self, cb: SendBoxFnOnce<'static, (SessionDescription,)>) {
        let _ = self.sender.send(RtcThreadEvent::CreateAnswer(cb));
    }
    pub fn add_stream(&self, stream: &MediaStreamId) {
        let _ = self.sender.send(RtcThreadEvent::AddStream(stream.clone()));
    }

    /// This should not be invoked by clients
    pub fn internal_event(&self, event: InternalEvent) {
        let _ = self.sender.send(RtcThreadEvent::InternalEvent(event));
    }

    pub fn quit(&self) {
        let _ = self.sender.send(RtcThreadEvent::Quit);
    }
}

pub enum RtcThreadEvent {
    ConfigureStun(String, BundlePolicy),
    SetRemoteDescription(SessionDescription, SendBoxFnOnce<'static, ()>),
    SetLocalDescription(SessionDescription, SendBoxFnOnce<'static, ()>),
    AddIceCandidate(IceCandidate),
    CreateOffer(SendBoxFnOnce<'static, (SessionDescription,)>),
    CreateAnswer(SendBoxFnOnce<'static, (SessionDescription,)>),
    AddStream(MediaStreamId),
    InternalEvent(InternalEvent),
    Quit,
}

/// To allow everything to occur on the event loop,
/// the backend may need to send signals to itself
///
/// This is a somewhat leaky abstraction, but we don't
/// plan on having too many backends anyway
pub enum InternalEvent {
    OnNegotiationNeeded,
    OnIceCandidate(IceCandidate),
    OnAddStream(MediaStreamId),
    DescriptionAdded(
        SendBoxFnOnce<'static, ()>,
        DescriptionType,
        SdpType,
        /* remote offer generation */ u32,
    ),
    UpdateSignalingState,
    UpdateGatheringState,
    UpdateIceConnectionState,
}

pub fn handle_rtc_event(controller: &mut WebRtcControllerBackend, event: RtcThreadEvent) -> bool {
    let result = match event {
        RtcThreadEvent::ConfigureStun(server, policy) => controller.configure(&server, policy),
        RtcThreadEvent::SetRemoteDescription(desc, cb) => {
            controller.set_remote_description(desc, cb)
        }
        RtcThreadEvent::SetLocalDescription(desc, cb) => controller.set_local_description(desc, cb),
        RtcThreadEvent::AddIceCandidate(candidate) => controller.add_ice_candidate(candidate),
        RtcThreadEvent::CreateOffer(cb) => controller.create_offer(cb),
        RtcThreadEvent::CreateAnswer(cb) => controller.create_answer(cb),
        RtcThreadEvent::AddStream(media) => controller.add_stream(&media),
        RtcThreadEvent::InternalEvent(e) => controller.internal_event(e),
        RtcThreadEvent::Quit => {
            controller.quit();
            return false;
        }
    };
    if let Err(e) = result {
        error!("WebRTC backend encountered error: {:?}", e);
    }
    true
}
