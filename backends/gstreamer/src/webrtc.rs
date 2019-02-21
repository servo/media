use boxfnonce::SendBoxFnOnce;
use glib;
use glib::prelude::*;
use gst;
use gst::prelude::*;
use gst_sdp;
use gst_webrtc;
use media_stream::{GStreamerMediaStream};
use servo_media_webrtc::thread::InternalEvent;
use servo_media_webrtc::WebRtcController as WebRtcThread;
use servo_media_webrtc::*;
use servo_media_streams::MediaStream;
use std::sync::{Arc, Mutex};

// TODO:
// - add a proper error enum
// - figure out purpose of glib loop

pub struct GStreamerWebRtcController {
    webrtc: Option<gst::Element>,
    pipeline: gst::Pipeline,
    has_streams: bool,
    delayed_negotiation: bool,
    thread: WebRtcThread,
    signaller: Box<WebRtcSignaller>,
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
        self.set_description(desc, false, cb);
    }

    fn set_local_description(&mut self, desc: SessionDescription, cb: SendBoxFnOnce<'static, ()>) {
        self.set_description(desc, true, cb);
    }

    fn create_offer(&mut self, cb: SendBoxFnOnce<'static, (SessionDescription,)>) {
        let webrtc = self.webrtc.as_ref().unwrap();
        let promise = gst::Promise::new_with_change_func(move |promise| {
            on_offer_or_answer_created(SdpType::Offer, promise, cb);
        });

        webrtc
            .emit("create-offer", &[&None::<gst::Structure>, &promise])
            .unwrap();
    }

    fn create_answer(&mut self, cb: SendBoxFnOnce<'static, (SessionDescription,)>) {
        let webrtc = self.webrtc.as_ref().unwrap();
        let promise = gst::Promise::new_with_change_func(move |promise| {
            on_offer_or_answer_created(SdpType::Answer, promise, cb);
        });

        webrtc
            .emit("create-answer", &[&None::<gst::Structure>, &promise])
            .unwrap();
    }

    fn add_stream(&mut self, stream: &mut MediaStream) {
        println!("adding a stream");
        self.has_streams = true;
        let stream = stream
            .as_mut_any()
            .downcast_mut::<GStreamerMediaStream>()
            .unwrap();
        stream.attach_to_webrtc(&self.pipeline, self.webrtc.as_ref().unwrap());
        self.pipeline
            .set_state(gst::State::Playing)
            .unwrap();
        if self.delayed_negotiation {
            self.delayed_negotiation = false;
            self.signaller.on_negotiation_needed(&self.thread);
        }
    }

    fn configure(&mut self, stun_server: &str, policy: BundlePolicy) {
        let webrtc = self.webrtc.as_ref().unwrap();
        webrtc.set_property_from_str("stun-server", stun_server);
        webrtc.set_property_from_str("bundle-policy", policy.as_str());
    }

    fn internal_event(&mut self, e: thread::InternalEvent) {
        match e {
            InternalEvent::OnNegotiationNeeded => {
                if self.has_streams {
                    self.signaller.on_negotiation_needed(&self.thread);
                } else {
                    // If the pipeline starts playing and on-negotiation-needed is present before there are any
                    // media streams, an invalid SDP offer will be created. Therefore, delay emitting the signal
                    self.delayed_negotiation = true;
                }
            }
            InternalEvent::OnIceCandidate(candidate) => {
                self.signaller.on_ice_candidate(&self.thread, candidate);
            }
            InternalEvent::OnAddStream(stream) => {
                self.pipeline
                    .set_state(gst::State::Playing)
                    .unwrap();
                self.signaller.on_add_stream(stream);
            }
        }
    }

    fn quit(&mut self) {
        self.signaller.close();

        self.pipeline.set_state(gst::State::Null).unwrap();

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
            SdpType::Answer => gst_webrtc::WebRTCSDPType::Answer,
            SdpType::Offer => gst_webrtc::WebRTCSDPType::Offer,
            SdpType::Pranswer => gst_webrtc::WebRTCSDPType::Pranswer,
            SdpType::Rollback => gst_webrtc::WebRTCSDPType::Rollback,
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
        let thread = Arc::new(Mutex::new(self.thread.clone()));
        webrtc
            .connect("pad-added", false, move |values| {
                println!("pad-added");
                process_new_stream(values, &pipe_clone, thread.clone());
                None
            })
            .unwrap();

        // gstreamer needs Sync on these callbacks for some reason
        // https://github.com/sdroege/gstreamer-rs/issues/154
        let thread = Mutex::new(self.thread.clone());
        webrtc
            .connect("on-negotiation-needed", false, move |_values| {
                thread
                    .lock()
                    .unwrap()
                    .internal_event(InternalEvent::OnNegotiationNeeded);
                None
            })
            .unwrap();


        self.webrtc = Some(webrtc);

        self.pipeline
            .set_state(gst::State::Ready)
            .unwrap();
    }
}

pub fn construct(
    signaller: Box<WebRtcSignaller>,
    thread: WebRtcThread,
) -> GStreamerWebRtcController {
    let main_loop = glib::MainLoop::new(None, false);
    let pipeline = gst::Pipeline::new("webrtc main");

    let mut controller = GStreamerWebRtcController {
        webrtc: None,
        pipeline,
        signaller,
        thread,
        has_streams: false,
        delayed_negotiation: false,
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
        gst_webrtc::WebRTCSDPType::Answer => SdpType::Answer,
        gst_webrtc::WebRTCSDPType::Offer => SdpType::Offer,
        gst_webrtc::WebRTCSDPType::Pranswer => SdpType::Pranswer,
        gst_webrtc::WebRTCSDPType::Rollback => SdpType::Rollback,
        _ => panic!("unknown sdp response"),
    };

    let desc = SessionDescription {
        sdp: reply.get_sdp().as_text().unwrap(),
        type_,
    };
    cb.call(desc);
}
fn on_incoming_stream(
    pipe: &gst::Pipeline,
    thread: Arc<Mutex<WebRtcThread>>,
    pad: &gst::Pad,
) {
    let decodebin = gst::ElementFactory::make("decodebin", None).unwrap();
    let pipe_clone = pipe.clone();
    let caps = pad.query_caps(None).unwrap();
    let name = caps.get_structure(0).unwrap().get::<String>("media").unwrap();
    let decodebin2 = decodebin.clone();
    decodebin
        .connect("pad-added", false, move |values| {
            println!("decodebin pad-added");
            on_incoming_decodebin_stream(values, &pipe_clone, thread.clone(), &name);
            None
        })
        .unwrap();
    decodebin
        .connect("no-more-pads", false, move |_| {
            println!("no-more-pads");
            None
        })
        .unwrap();
    pipe.add(&decodebin).unwrap();

    let decodepad = decodebin.get_static_pad("sink").unwrap();
    pad.link(&decodepad).unwrap();
    decodebin2.sync_state_with_parent().unwrap();
}

fn on_incoming_decodebin_stream(
    values: &[glib::Value],
    pipe: &gst::Pipeline,
    thread: Arc<Mutex<WebRtcThread>>,
    name: &str,
) {
    println!("incoming decodebin");
    let pad = values[1].get::<gst::Pad>().expect("not a pad??");
    let proxy_src = gst::ElementFactory::make("proxysrc", None).unwrap();
    let proxy_sink = gst::ElementFactory::make("proxysink", None).unwrap();
    proxy_src.set_property("proxysink", &proxy_sink).unwrap();
    pipe.add(&proxy_sink).unwrap();
    let sinkpad = proxy_sink.get_static_pad("sink").unwrap();

    pad.link(&sinkpad).unwrap();
    proxy_sink.sync_state_with_parent().unwrap();

    let stream = if name == "video" {
        Box::new(GStreamerMediaStream::create_video_from(proxy_src))
    } else {
        Box::new(GStreamerMediaStream::create_audio_from(proxy_src))
    };
    thread
        .lock()
        .unwrap()
        .internal_event(InternalEvent::OnAddStream(stream));
}

fn process_new_stream(
    values: &[glib::Value],
    pipe: &gst::Pipeline,
    thread: Arc<Mutex<WebRtcThread>>,
) {
    let pad = values[1].get::<gst::Pad>().expect("not a pad??");
    if pad.get_direction() != gst::PadDirection::Src {
        // Ignore outgoing pad notifications.
        return;
    }
    on_incoming_stream(pipe, thread, &pad)
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
