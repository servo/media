use failure::Error;
use glib::{self, ObjectExt};
use gst::{self, ElementExt, BinExt, PadExt, BinExtManual, GObjectExtManualGst};
use gst_sdp;
use gst_webrtc;
use servo_media_webrtc::*;
use std::sync::{Arc, Mutex};

// TODO:
// - configurable STUN server?
// - remove use of failure?
// - figure out purpose of glib loop

const STUN_SERVER: &str = "stun://stun.l.google.com:19302";
lazy_static! {
    static ref RTP_CAPS_OPUS: gst::Caps = {
        gst::Caps::new_simple(
            "application/x-rtp",
            &[
                ("media", &"audio"),
                ("encoding-name", &"OPUS"),
                ("payload", &(97i32)),
            ],
        )
    };
    static ref RTP_CAPS_VP8: gst::Caps = {
        gst::Caps::new_simple(
            "application/x-rtp",
            &[
                ("media", &"video"),
                ("encoding-name", &"VP8"),
                ("payload", &(96i32)),
            ],
        )
    };
}

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
    PeerCallStarted,
    PeerCallError,
}

#[derive(Clone)]
pub struct GStreamerWebRtcController(Arc<Mutex<WebRtcControllerState>>);

impl WebRtcController for GStreamerWebRtcController {
    fn trigger_negotiation(&self) {
        let app_control = self.0.lock().unwrap();
        app_control
            .webrtc
            .as_ref()
            .unwrap()
            .emit("on-negotiation-needed", &[])
            .unwrap();
    }

    fn notify_signal_server_error(&self) {
        //TODO
    }

    fn add_ice_candidate(&self, candidate: IceCandidate) {
        let app_control = self.0.lock().unwrap();
        app_control
            .webrtc
            .as_ref()
            .unwrap()
            .emit("add-ice-candidate", &[&candidate.sdp_mline_index, &candidate.candidate])
            .unwrap();
    }

    fn set_remote_description(&self, desc: SessionDescription) {
        if !self.assert_app_state_is(AppState::PeerCallNegotiating, "Not ready to handle sdp") {
            return;
        }

        self.set_description(desc, false);

        let mut app_control = self.0.lock().unwrap();
        app_control.app_state = AppState::PeerCallStarted;
    }

    fn set_local_description(&self, desc: SessionDescription) {
        if !self.assert_app_state_is(AppState::PeerCallNegotiating, "Not ready to handle sdp") {
            return;
        }

        self.set_description(desc, true);
    }
}

impl GStreamerWebRtcController {
    fn start_pipeline(&self) -> Result<(), Error> {
        self.0.lock().unwrap().start_pipeline(self.clone())
    }

    fn set_description(&self, desc: SessionDescription, local: bool) {
        use gst_webrtc::WebRTCSDPType;

        let ty = match desc.type_ {
            SdpType::Answer => WebRTCSDPType::Answer,
            SdpType::Offer => WebRTCSDPType::Offer,
            SdpType::Pranswer => WebRTCSDPType::Pranswer,
            SdpType::Rollback => WebRTCSDPType::Rollback,
        };

        let kind = if local { "set-local-description" } else { "set-remote-description" };

        let mut app_control = self.0.lock().unwrap();
        let ret = gst_sdp::SDPMessage::parse_buffer(desc.sdp.as_bytes()).unwrap();
        let answer =
            gst_webrtc::WebRTCSessionDescription::new(ty, ret);
        let promise = gst::Promise::new();
        app_control
            .webrtc
            .as_ref()
            .unwrap()
            .emit(kind, &[&answer, &promise])
            .unwrap();
    }

    fn assert_app_state_is(&self, state: AppState, error_msg: &'static str) -> bool {
        if self.0.lock().unwrap().app_state != state {
            self.send_bus_error(error_msg);

            false
        } else {
            true
        }
    }

    fn assert_app_state_is_at_least(&self, state: AppState, error_msg: &'static str) -> bool {
        if self.0.lock().unwrap().app_state < state {
            self.send_bus_error(error_msg);

            false
        } else {
            true
        }
    }

    //#[allow(unused)]
    fn send_bus_error(&self, body: &str) {
        eprintln!("Bus error: {}", body);
        /*let mbuilder =
            gst::Message::new_application(gst::Structure::new("error", &[("body", &body)]));
        let _ = self.0.lock().unwrap().bus.post(&mbuilder.build());*/
        //XXXjdm
    }

    #[allow(unused)]
    fn update_state(&self, state: AppState) {
        self.0.lock().unwrap().update_state(state);
    }

    #[allow(unused)]
    fn close_and_quit(&self, err: &Error) {
        println!("{}\nquitting", err);

        // Must not hold mutex while shutting down the pipeline
        // as something might call into here and take the mutex too
        let pipeline = {
            let app_control = self.0.lock().unwrap();
            app_control.signaller.close(err.to_string());
            app_control.pipeline.clone()
        };

        pipeline.set_state(gst::State::Null).into_result().unwrap();

        //main_loop.quit();
    }
}

struct WebRtcControllerState {
    webrtc: Option<gst::Element>,
    app_state: AppState,
    pipeline: gst::Pipeline,
    signaller: Box<WebRtcSignaller>,
    //send_msg_tx: mpsc::Sender<OwnedMessage>,
    //peer_id: String,
    _main_loop: glib::MainLoop,
    //bus: gst::Bus,
}

impl WebRtcControllerState {
    fn construct_pipeline(&self) -> Result<gst::Pipeline, Error> {
        let pipeline = self.pipeline.clone();

        let webrtcbin = gst::ElementFactory::make("webrtcbin", "sendrecv").unwrap();
        pipeline.add(&webrtcbin)?;

        webrtcbin.set_property_from_str("stun-server", STUN_SERVER);
        webrtcbin.set_property_from_str("bundle-policy", "max-bundle");

        add_video_source(&pipeline, &webrtcbin)?;
        add_audio_source(&pipeline, &webrtcbin)?;

        Ok(pipeline)
    }

    fn start_pipeline(&mut self, target: GStreamerWebRtcController) -> Result<(), Error> {
        let pipe = self.construct_pipeline()?;
        let webrtc = pipe.get_by_name("sendrecv").unwrap();

        let app_control_clone = target.clone();
        webrtc.connect("on-negotiation-needed", false, move |values| {
            on_negotiation_needed(&app_control_clone, values).unwrap();
            None
        })?;

        let app_control_clone = target.clone();
        webrtc.connect("on-ice-candidate", false, move |values| {
            send_ice_candidate_message(&app_control_clone, values);
            None
        })?;

        let pipe_clone = pipe.clone();
        let app_control_clone = target.clone();
        webrtc.connect("pad-added", false, move |values| {
            on_incoming_stream(&app_control_clone, values, &pipe_clone)
        })?;

        pipe.set_state(gst::State::Playing).into_result()?;

        self.webrtc = Some(webrtc);

        Ok(())
    }

    fn update_state(&mut self, state: AppState) {
        self.app_state = state;
    }
}

pub fn start(signaller: Box<WebRtcSignaller>) -> GStreamerWebRtcController {
    let main_loop = glib::MainLoop::new(None, false);
    let pipeline = gst::Pipeline::new("main");
    //let bus = pipeline.get_bus().unwrap();

    let controller = WebRtcControllerState {
        webrtc: None,
        pipeline,
        signaller,
        app_state: AppState::ServerConnected,
        _main_loop: main_loop,
    };
    let controller = GStreamerWebRtcController(Arc::new(Mutex::new(controller)));
    controller.start_pipeline().unwrap();

    let controller_clone = controller.clone();
            
    /*bus.add_watch(move |_, msg| {
        use gst::message::MessageView;

        match msg.view() {
            MessageView::Error(err) => controller.close_and_quit(&Error::from(err.get_error())),
            MessageView::Warning(warning) => {
                println!("Warning: \"{}\"", warning.get_debug().unwrap());
            }
            MessageView::Application(a) => {
                let struc = a.get_structure().unwrap();
                if let Err(err) = handle_application_msg(&controller, struc) {
                    controller.close_and_quit(&err)
                }
            }
            _ => {}
        };

        glib::Continue(true)
    });*/

    controller_clone
}

/*fn handle_application_msg(
    app_control: &GStreamerWebRtcController,
    struc: &gst::StructureRef,
) -> Result<(), Error> {
    match struc.get_name() {
        "ws-message" => {
            let msg = struc.get_value("body").unwrap();
            app_control.on_message(msg.get().unwrap())
        }
        "ws-error" => Err(WsError(app_control.0.lock().unwrap().app_state))?,
        "error" => {
            let msg: String = struc.get_value("body").unwrap().get().unwrap();
            Err(BusError(msg))?
        }
        u => {
            println!("Got unknown application message {:?}", u);

            Ok(())
        }
    }
}*/

fn add_video_source(pipeline: &gst::Pipeline, webrtcbin: &gst::Element) -> Result<(), Error> {
    let videotestsrc = gst::ElementFactory::make("videotestsrc", None).unwrap();
    let videoconvert = gst::ElementFactory::make("videoconvert", None).unwrap();
    let queue = gst::ElementFactory::make("queue", None).unwrap();
    let vp8enc = gst::ElementFactory::make("vp8enc", None).unwrap();

    videotestsrc.set_property_from_str("pattern", "ball");
    videotestsrc.set_property("is-live", &true).unwrap();
    vp8enc.set_property("deadline", &1i64).unwrap();

    let rtpvp8pay = gst::ElementFactory::make("rtpvp8pay", None).unwrap();
    let queue2 = gst::ElementFactory::make("queue", None).unwrap();

    pipeline.add_many(&[
        &videotestsrc,
        &videoconvert,
        &queue,
        &vp8enc,
        &rtpvp8pay,
        &queue2,
    ])?;

    gst::Element::link_many(&[
        &videotestsrc,
        &videoconvert,
        &queue,
        &vp8enc,
        &rtpvp8pay,
        &queue2,
    ])?;

    queue2.link_filtered(webrtcbin, &*RTP_CAPS_VP8)?;

    Ok(())
}

fn add_audio_source(pipeline: &gst::Pipeline, webrtcbin: &gst::Element) -> Result<(), Error> {
    let audiotestsrc = gst::ElementFactory::make("audiotestsrc", None).unwrap();
    let queue = gst::ElementFactory::make("queue", None).unwrap();
    let audioconvert = gst::ElementFactory::make("audioconvert", None).unwrap();
    let audioresample = gst::ElementFactory::make("audioresample", None).unwrap();
    let queue2 = gst::ElementFactory::make("queue", None).unwrap();
    let opusenc = gst::ElementFactory::make("opusenc", None).unwrap();
    let rtpopuspay = gst::ElementFactory::make("rtpopuspay", None).unwrap();
    let queue3 = gst::ElementFactory::make("queue", None).unwrap();

    audiotestsrc.set_property_from_str("wave", "red-noise");
    audiotestsrc.set_property("is-live", &true).unwrap();

    pipeline.add_many(&[
        &audiotestsrc,
        &queue,
        &audioconvert,
        &audioresample,
        &queue2,
        &opusenc,
        &rtpopuspay,
        &queue3,
    ])?;

    gst::Element::link_many(&[
        &audiotestsrc,
        &queue,
        &audioconvert,
        &audioresample,
        &queue2,
        &opusenc,
        &rtpopuspay,
        &queue3,
    ])?;

    queue3.link_filtered(webrtcbin, &*RTP_CAPS_OPUS)?;

    Ok(())
}

fn send_sdp_offer(app_control: &GStreamerWebRtcController, offer: &gst_webrtc::WebRTCSessionDescription) {
    if !app_control.assert_app_state_is_at_least(
        AppState::PeerCallNegotiating,
        "Can't send offer, not in call",
    ) {
        return;
    }

    app_control.0.lock().unwrap().signaller.send_sdp_offer(
        offer.get_sdp().as_text().unwrap()
    );
}

fn on_offer_created(
    app_control: &GStreamerWebRtcController,
    webrtc: &gst::Element,
    promise: &gst::Promise,
) -> Result<(), Error> {
    if !app_control.assert_app_state_is(
        AppState::PeerCallNegotiating,
        "Not negotiating call when creating offer",
    ) {
        return Ok(());
    }

    let reply = promise.get_reply().unwrap();

    let offer = reply
        .get_value("offer")
        .unwrap()
        .get::<gst_webrtc::WebRTCSessionDescription>()
        .expect("Invalid argument");
    webrtc.emit("set-local-description", &[&offer, &None::<gst::Promise>])?;

    send_sdp_offer(&app_control, &offer);

    Ok(())
}

fn on_negotiation_needed(app_control: &GStreamerWebRtcController, values: &[glib::Value]) -> Result<(), Error> {
    app_control.0.lock().unwrap().app_state = AppState::PeerCallNegotiating;

    let webrtc = values[0].get::<gst::Element>().unwrap();
    let webrtc_clone = webrtc.clone();
    let app_control_clone = app_control.clone();
    let promise = gst::Promise::new_with_change_func(move |promise| {
        on_offer_created(&app_control_clone, &webrtc, promise).unwrap();
    });

    webrtc_clone.emit("create-offer", &[&None::<gst::Structure>, &promise])?;

    Ok(())
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
    app_control: &GStreamerWebRtcController,
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
        app_control.send_bus_error(&format!("Error adding pad with caps {} {:?}", name, err));
    }

    None
}

fn on_incoming_stream(
    app_control: &GStreamerWebRtcController,
    values: &[glib::Value],
    pipe: &gst::Pipeline,
) -> Option<glib::Value> {
    let webrtc = values[0].get::<gst::Element>().expect("Invalid argument");

    let decodebin = gst::ElementFactory::make("decodebin", None).unwrap();
    let pipe_clone = pipe.clone();
    let app_control_clone = app_control.clone();
    decodebin
        .connect("pad-added", false, move |values| {
            on_incoming_decodebin_stream(&app_control_clone, values, &pipe_clone)
        })
        .unwrap();

    pipe.add(&decodebin).unwrap();

    decodebin.sync_state_with_parent().unwrap();
    webrtc.link(&decodebin).unwrap();

    None
}

fn send_ice_candidate_message(app_control: &GStreamerWebRtcController, values: &[glib::Value]) {
    if !app_control
        .assert_app_state_is_at_least(AppState::PeerCallNegotiating, "Can't send ICE, not in call")
    {
        return;
    }

    let _webrtc = values[0].get::<gst::Element>().expect("Invalid argument");
    let sdp_mline_index = values[1].get::<u32>().expect("Invalid argument");
    let candidate = values[2].get::<String>().expect("Invalid argument");

    let candidate = IceCandidate {
        sdp_mline_index,
        candidate,
    };
    app_control.0.lock().unwrap().signaller.on_ice_candidate(candidate);
}
