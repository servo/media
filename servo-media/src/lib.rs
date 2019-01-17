pub extern crate servo_media_audio as audio;
#[cfg(any(
    all(target_os = "android", target_arch = "arm"),
    target_arch = "x86_64"
))]
extern crate servo_media_gstreamer;
pub extern crate servo_media_player as player;
pub extern crate servo_media_webrtc as webrtc;
use std::sync::{self, Arc, Mutex, Once};

use audio::context::{AudioContext, AudioContextOptions};
use audio::decoder::DummyAudioDecoder;
use audio::sink::{AudioSinkError, DummyAudioSink};
use audio::AudioBackend;
use player::{DummyPlayer, Player, PlayerBackend};
use webrtc::{WebRtcBackend, WebRtcSignaller};

pub struct ServoMedia;

static INITIALIZER: Once = sync::ONCE_INIT;
static mut INSTANCE: *mut Mutex<Option<Arc<ServoMedia>>> = 0 as *mut _;

pub struct DummyMediaStream;
pub struct DummyBackend {}

impl AudioBackend for DummyBackend {
    type Decoder = DummyAudioDecoder;
    type Sink = DummyAudioSink;
    fn make_decoder() -> Self::Decoder {
        DummyAudioDecoder
    }

    fn make_sink() -> Result<Self::Sink, AudioSinkError> {
        Ok(DummyAudioSink)
    }
}

impl PlayerBackend for DummyBackend {
    type Player = DummyPlayer;
    fn make_player() -> Self::Player {
        DummyPlayer {}
    }
}

impl DummyBackend {
    pub fn init() {}
    pub fn create_mediastream() -> DummyMediaStream {
        DummyMediaStream
    }
}

#[cfg(any(
    all(target_os = "android", target_arch = "arm"),
    target_arch = "x86_64"
))]
pub type Backend = servo_media_gstreamer::GStreamerBackend;
#[cfg(not(any(
    all(target_os = "android", target_arch = "arm"),
    target_arch = "x86_64"
)))]
pub type Backend = DummyBackend;

pub type WebRtcController = servo_media_gstreamer::webrtc::GStreamerWebRtcController;

pub type MediaStream = servo_media_gstreamer::media_stream::GStreamerMediaStream;

impl ServoMedia {
    pub fn new() -> Self {
        Backend::init();

        Self {}
    }

    pub fn get() -> Result<Arc<ServoMedia>, ()> {
        INITIALIZER.call_once(|| unsafe {
            INSTANCE = Box::into_raw(Box::new(Mutex::new(Some(Arc::new(ServoMedia::new())))));
        });
        let instance = unsafe { &*INSTANCE }.lock().unwrap();
        match *instance {
            Some(ref instance) => Ok(instance.clone()),
            None => Err(()),
        }
    }

    pub fn create_audio_context(&self, options: AudioContextOptions) -> AudioContext<Backend> {
        AudioContext::new(options)
    }

    pub fn create_player(&self) -> Box<Player> {
        Box::new(Backend::make_player())
    }

    pub fn create_webrtc(&self, signaller: Box<WebRtcSignaller>) -> Box<WebRtcController> {
        Box::new(Backend::start_webrtc_controller(signaller))
    }

    pub fn create_mediastream(&self) -> Box<MediaStream> {
        Box::new(Backend::create_mediastream())
    }
}
