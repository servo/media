extern crate boxfnonce;
extern crate byte_slice_cast;

#[macro_use]
extern crate glib;
extern crate glib_sys as glib_ffi;
extern crate gobject_sys as gobject_ffi;
#[macro_use]
extern crate gobject_subclass;
extern crate gst_plugin;
extern crate gstreamer as gst;
extern crate gstreamer_app as gst_app;
extern crate gstreamer_audio as gst_audio;
extern crate gstreamer_player as gst_player;
extern crate gstreamer_sdp as gst_sdp;
extern crate gstreamer_sys as gst_ffi;
extern crate gstreamer_video as gst_video;
extern crate gstreamer_webrtc as gst_webrtc;
extern crate ipc_channel;
#[macro_use]
extern crate lazy_static;

extern crate servo_media_audio;
extern crate servo_media_player;
extern crate servo_media_webrtc;
extern crate url;

use servo_media_audio::sink::AudioSinkError;
use servo_media_audio::AudioBackend;
use servo_media_player::PlayerBackend;
use servo_media_webrtc::{WebRtcBackend, WebRtcController, WebRtcSignaller};

pub mod audio_decoder;
pub mod audio_sink;
pub mod media_capture;
pub mod media_stream;
pub mod player;
mod source;
pub mod webrtc;

pub struct GStreamerBackend;

impl AudioBackend for GStreamerBackend {
    type Decoder = audio_decoder::GStreamerAudioDecoder;
    type Sink = audio_sink::GStreamerAudioSink;
    fn make_decoder() -> Self::Decoder {
        audio_decoder::GStreamerAudioDecoder::new()
    }
    fn make_sink() -> Result<Self::Sink, AudioSinkError> {
        audio_sink::GStreamerAudioSink::new()
    }
}

impl PlayerBackend for GStreamerBackend {
    type Player = player::GStreamerPlayer;
    fn make_player() -> Self::Player {
        player::GStreamerPlayer::new()
    }
}

impl WebRtcBackend for GStreamerBackend {
    type Controller = webrtc::GStreamerWebRtcController;

    fn construct_webrtc_controller(
        signaller: Box<WebRtcSignaller>,
        thread: WebRtcController,
    ) -> Self::Controller {
        webrtc::construct(signaller, thread)
    }
}

impl GStreamerBackend {
    pub fn init() {
        gst::init().unwrap();
    }

    pub fn create_audiostream() -> media_stream::GStreamerMediaStream {
        media_stream::GStreamerMediaStream::create_audio()
    }

    pub fn create_videostream() -> media_stream::GStreamerMediaStream {
        media_stream::GStreamerMediaStream::create_video()
    }

    pub fn create_audioinput_stream() -> Option<media_stream::GStreamerMediaStream> {
        media_capture::create_audioinput_stream()
    }

    pub fn create_videoinput_stream() -> Option<media_stream::GStreamerMediaStream> {
        media_capture::create_videoinput_stream()
    }
}
