use boxfnonce::SendBoxFnOnce;
use failure::Error;
use glib::{self, ObjectExt};
use gst::{self, BinExt, BinExtManual, ElementExt, GObjectExtManualGst, PadDirection, PadExt};
use gst_sdp;
use gst_webrtc::{self, WebRTCSDPType};
use media_stream::GStreamerMediaStream;
use servo_media_webrtc::thread::InternalEvent;
use servo_media_webrtc::WebRtcController as WebRtcThread;
use servo_media_webrtc::*;
use std::sync::Mutex;

// TODO:
// - remove use of failure?
// - figure out purpose of glib loop

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum MediaType {
    Audio,
    Video,
}

#[derive(PartialEq, PartialOrd, Eq, Debug, Copy, Clone, Ord)]
#[allow(unused)]
enum AppState {
    Error = 1,
    ServerConnected,
    ServerRegistering = 2000,
    ServerRegisteringError,
    ServerRegistered,
    PeerConnecting = 3000,
    PeerConnectionError,
    PeerConnected,
    PeerCallNegotiating = 4000,
    PeerCallNegotiatingHaveLocal,
    PeerCallNegotiatingHaveRemote,
    PeerCallStarted,
    PeerCallError,
}

pub struct GStreamerWebRtcController {
    webrtc: Option<gst::Element>,
    app_state: AppState,
    pipeline: gst::Pipeline,
    thread: WebRtcThread,
    signaller: Box<WebRtcSignaller>,
    ready_to_negotiate: bool,
    //send_msg_tx: mpsc::Sender<OwnedMessage>,
    //peer_id: String,
    _main_loop: glib::MainLoop,
    //bus: gst::Bus,
}

impl WebRtcControllerBackend for GStreamerWebRtcController {
    fn add_ice_candidate(&mut self, candidate: IceCandidate) {
        self.webrtc
            .as_ref()
            .unwrap()
            .emit(
                "add-ice-candidate",
                &[&candidate.sdp_mline_index, &candidate.candidate],
            )
            .unwrap();
    }

    fn set_remote_description(&mut self, desc: SessionDescription, cb: SendBoxFnOnce<'static, ()>) {
        assert!(
            self.app_state == AppState::PeerCallNegotiating
                || self.app_state == AppState::PeerCallNegotiatingHaveLocal,
            "Not ready to handle sdp"
        );

        self.set_description(desc, false, cb);

        if self.app_state == AppState::PeerCallNegotiating {
            self.app_state = AppState::PeerCallNegotiatingHaveRemote;
        } else {
            self.app_state = AppState::PeerCallStarted;
        }
    }

    fn set_local_description(&mut self, desc: SessionDescription, cb: SendBoxFnOnce<'static, ()>) {
        assert!(
            self.app_state == AppState::PeerCallNegotiating
                || self.app_state == AppState::PeerCallNegotiatingHaveRemote,
            "Not ready to handle sdp"
        );

        self.set_description(desc, true, cb);

        if self.app_state == AppState::PeerCallNegotiating {
            self.app_state = AppState::PeerCallNegotiatingHaveLocal;
        } else {
            self.app_state = AppState::PeerCallStarted;
        }
    }

    fn create_offer(&mut self, cb: SendBoxFnOnce<'static, (SessionDescription,)>) {
        let webrtc = self.webrtc.as_ref().unwrap();
        assert!(
            self.app_state == AppState::PeerCallNegotiating,
            "Not negotiating call when creating offer"
        );
        let promise = gst::Promise::new_with_change_func(move |promise| {
            on_offer_or_answer_created(SdpType::Offer, promise, cb);
        });

        webrtc
            .emit("create-offer", &[&None::<gst::Structure>, &promise])
            .unwrap();
    }

    fn create_answer(&mut self, cb: SendBoxFnOnce<'static, (SessionDescription,)>) {
        let webrtc = self.webrtc.as_ref().unwrap();
        assert!(
            self.app_state == AppState::PeerCallNegotiatingHaveRemote,
            "No offfer received when creating answer"
        );
        let promise = gst::Promise::new_with_change_func(move |promise| {
            on_offer_or_answer_created(SdpType::Answer, promise, cb);
        });

        webrtc
            .emit("create-answer", &[&None::<gst::Structure>, &promise])
            .unwrap();
    }

    fn add_stream(&mut self, stream: &MediaStream) {
        println!("adding a stream");
        let stream = stream
            .as_any()
            .downcast_ref::<GStreamerMediaStream>()
            .unwrap();
        stream.attach_to_pipeline(&self.pipeline, self.webrtc.as_ref().unwrap());
        self.prepare_for_negotiation();
    }

    fn configure(&mut self, stun_server: &str, policy: BundlePolicy) {
        let webrtc = self.webrtc.as_ref().unwrap();
        webrtc.set_property_from_str("stun-server", stun_server);
        webrtc.set_property_from_str("bundle-policy", policy.as_str());
    }

    fn internal_event(&mut self, e: thread::InternalEvent) {
        match e {
            InternalEvent::OnNegotiationNeeded => {
                self.app_state = AppState::PeerCallNegotiating;
                self.signaller.on_negotiation_needed(&self.thread);
            }
            InternalEvent::OnIceCandidate(candidate) => {
                self.signaller.on_ice_candidate(&self.thread, candidate);
            }
        }
    }

    fn quit(&mut self) {
        self.signaller.close();

        self.pipeline
            .set_state(gst::State::Null)
            .into_result()
            .unwrap();

        //main_loop.quit();
    }
}

impl GStreamerWebRtcController {
    fn set_description(
        &self,
        desc: SessionDescription,
        local: bool,
        cb: SendBoxFnOnce<'static, ()>,
    ) {
        let ty = match desc.type_ {
            SdpType::Answer => WebRTCSDPType::Answer,
            SdpType::Offer => WebRTCSDPType::Offer,
            SdpType::Pranswer => WebRTCSDPType::Pranswer,
            SdpType::Rollback => WebRTCSDPType::Rollback,
        };

        let kind = if local {
            "set-local-description"
        } else {
            "set-remote-description"
        };

        let ret = gst_sdp::SDPMessage::parse_buffer(desc.sdp.as_bytes()).unwrap();
        let answer = gst_webrtc::WebRTCSessionDescription::new(ty, ret);
        let promise = gst::Promise::new_with_change_func(move |_promise| cb.call());
        self.webrtc
            .as_ref()
            .unwrap()
            .emit(kind, &[&answer, &promise])
            .unwrap();
    }
}

impl GStreamerWebRtcController {
    fn prepare_for_negotiation(&mut self) {
        if self.ready_to_negotiate {
            return;
        }
        self.ready_to_negotiate = true;
        let webrtc = self.webrtc.as_ref().unwrap();
        // gstreamer needs Sync on these callbacks for some reason
        // https://github.com/sdroege/gstreamer-rs/issues/154
        let thread = Mutex::new(self.thread.clone());
        // If the pipeline starts playing and this signal is present before there are any
        // media streams, an invalid SDP offer will be created. Therefore, delay setting up
        // the signal and starting the pipeline until after the first stream has been added.
        webrtc
            .connect("on-negotiation-needed", false, move |_values| {
                thread
                    .lock()
                    .unwrap()
                    .internal_event(InternalEvent::OnNegotiationNeeded);
                None
            })
            .unwrap();
        self.pipeline
            .set_state(gst::State::Playing)
            .into_result()
            .unwrap();
    }

    fn start_pipeline(&mut self) {
        let webrtc = gst::ElementFactory::make("webrtcbin", "sendrecv").unwrap();
        self.pipeline.add(&webrtc).unwrap();

        // gstreamer needs Sync on these callbacks for some reason
        // https://github.com/sdroege/gstreamer-rs/issues/154
        let thread = Mutex::new(self.thread.clone());
        webrtc
            .connect("on-ice-candidate", false, move |values| {
                thread
                    .lock()
                    .unwrap()
                    .internal_event(InternalEvent::OnIceCandidate(candidate(values)));
                None
            })
            .unwrap();

        let pipe_clone = self.pipeline.clone();
        webrtc
            .connect("pad-added", false, move |values| {
                println!("pad-added");
                process_new_stream(values, &pipe_clone);
                None
            })
            .unwrap();

        self.webrtc = Some(webrtc);
    }
}

pub fn construct(
    signaller: Box<WebRtcSignaller>,
    thread: WebRtcThread,
) -> GStreamerWebRtcController {
    let main_loop = glib::MainLoop::new(None, false);
    let pipeline = gst::Pipeline::new("main");

    let mut controller = GStreamerWebRtcController {
        webrtc: None,
        pipeline,
        signaller,
        thread,
        app_state: AppState::ServerConnected,
        ready_to_negotiate: false,
        _main_loop: main_loop,
    };
    controller.start_pipeline();
    controller
}

fn on_offer_or_answer_created(
    ty: SdpType,
    promise: &gst::Promise,
    cb: SendBoxFnOnce<'static, (SessionDescription,)>,
) {
    debug_assert!(ty == SdpType::Offer || ty == SdpType::Answer);

    let reply = promise.get_reply().unwrap();

    let reply = reply
        .get_value(ty.as_str())
        .unwrap()
        .get::<gst_webrtc::WebRTCSessionDescription>()
        .expect("Invalid argument");

    let type_ = match reply.get_type() {
        WebRTCSDPType::Answer => SdpType::Answer,
        WebRTCSDPType::Offer => SdpType::Offer,
        WebRTCSDPType::Pranswer => SdpType::Pranswer,
        WebRTCSDPType::Rollback => SdpType::Rollback,
        _ => panic!("unknown sdp response"),
    };

    let desc = SessionDescription {
        sdp: reply.get_sdp().as_text().unwrap(),
        type_,
    };
    cb.call(desc);
}

fn handle_media_stream(
    pad: &gst::Pad,
    pipe: &gst::Pipeline,
    media_type: MediaType,
) -> Result<(), Error> {
    println!("Trying to handle stream {:?}", media_type);

    let (q, conv, sink) = match media_type {
        MediaType::Audio => {
            let q = gst::ElementFactory::make("queue", None).unwrap();
            let conv = gst::ElementFactory::make("audioconvert", None).unwrap();
            let sink = gst::ElementFactory::make("autoaudiosink", None).unwrap();
            let resample = gst::ElementFactory::make("audioresample", None).unwrap();

            pipe.add_many(&[&q, &conv, &resample, &sink])?;
            gst::Element::link_many(&[&q, &conv, &resample, &sink])?;

            resample.sync_state_with_parent()?;

            (q, conv, sink)
        }
        MediaType::Video => {
            let q = gst::ElementFactory::make("queue", None).unwrap();
            let conv = gst::ElementFactory::make("videoconvert", None).unwrap();
            let sink = gst::ElementFactory::make("autovideosink", None).unwrap();

            pipe.add_many(&[&q, &conv, &sink])?;
            gst::Element::link_many(&[&q, &conv, &sink])?;

            (q, conv, sink)
        }
    };
    q.sync_state_with_parent()?;
    conv.sync_state_with_parent()?;
    sink.sync_state_with_parent()?;

    let qpad = q.get_static_pad("sink").unwrap();
    pad.link(&qpad).into_result()?;

    Ok(())
}

fn on_incoming_decodebin_stream(
    values: &[glib::Value],
    pipe: &gst::Pipeline,
) -> Option<glib::Value> {
    let pad = values[1].get::<gst::Pad>().expect("Invalid argument");
    if !pad.has_current_caps() {
        println!("Pad {:?} has no caps, can't do anything, ignoring", pad);
        return None;
    }

    let caps = pad.get_current_caps().unwrap();
    let name = caps.get_structure(0).unwrap().get_name();

    let handled = if name.starts_with("video") {
        handle_media_stream(&pad, &pipe, MediaType::Video)
    } else if name.starts_with("audio") {
        handle_media_stream(&pad, &pipe, MediaType::Audio)
    } else {
        println!("Unknown pad {:?}, ignoring", pad);
        Ok(())
    };

    if let Err(err) = handled {
        eprintln!("Error adding pad with caps {} {:?}", name, err);
    }

    None
}

fn on_incoming_stream(values: &[glib::Value], pipe: &gst::Pipeline) -> Option<glib::Value> {
    let webrtc = values[0].get::<gst::Element>().expect("Invalid argument");

    let decodebin = gst::ElementFactory::make("decodebin", None).unwrap();
    let pipe_clone = pipe.clone();
    decodebin
        .connect("pad-added", false, move |values| {
            println!("decodebin pad-added");
            on_incoming_decodebin_stream(values, &pipe_clone)
        })
        .unwrap();

    pipe.add(&decodebin).unwrap();

    decodebin.sync_state_with_parent().unwrap();
    webrtc.link(&decodebin).unwrap();

    None
}

fn process_new_stream(values: &[glib::Value], pipe: &gst::Pipeline) -> Option<glib::Value> {
    let pad = values[1].get::<gst::Pad>().expect("not a pad??");
    if pad.get_direction() != PadDirection::Src {
        // Ignore outgoing pad notifications.
        return None;
    }
    on_incoming_stream(values, pipe)
}

fn candidate(values: &[glib::Value]) -> IceCandidate {
    let _webrtc = values[0].get::<gst::Element>().expect("Invalid argument");
    let sdp_mline_index = values[1].get::<u32>().expect("Invalid argument");
    let candidate = values[2].get::<String>().expect("Invalid argument");

    IceCandidate {
        sdp_mline_index,
        candidate,
    }
}
