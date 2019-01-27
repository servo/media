pub extern crate servo_media_audio as audio;
#[cfg(any(
    all(target_os = "android", target_arch = "arm"),
    target_arch = "x86_64"
))]
extern crate servo_media_gstreamer;
pub extern crate servo_media_player as player;
pub extern crate servo_media_webrtc as webrtc;
use std::any::Any;
use std::sync::{self, Arc, Mutex, Once};

use audio::context::{AudioContext, AudioContextOptions};
use audio::decoder::DummyAudioDecoder;
use audio::sink::{AudioSinkError, DummyAudioSink};
use audio::AudioBackend;
use player::{DummyPlayer, Player, PlayerBackend};
use webrtc::{DummyMediaOutput, DummyWebRtcController, WebRtcBackend};
use webrtc::{MediaStream, MediaOutput, WebRtcController, WebRtcSignaller};

pub struct ServoMedia;

static INITIALIZER: Once = sync::ONCE_INIT;
static mut INSTANCE: *mut Mutex<Option<Arc<ServoMedia>>> = 0 as *mut _;

pub struct DummyMediaStream;
impl MediaStream for DummyMediaStream {
    fn as_any(&self) -> &Any {
        self
    }
    fn as_mut_any(&mut self) -> &mut Any {
        self
    }
}

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

impl WebRtcBackend for DummyBackend {
    type Controller = DummyWebRtcController;
    fn construct_webrtc_controller(
        _: Box<WebRtcSignaller>,
        _: WebRtcController,
    ) -> Self::Controller {
        DummyWebRtcController
    }
}

impl DummyBackend {
    pub fn init() {}
    pub fn create_audiostream() -> DummyMediaStream {
        DummyMediaStream
    }

    pub fn create_videostream() -> DummyMediaStream {
        DummyMediaStream
    }

    pub fn create_strema_output() -> DummyMediaOutput {
        DummyMediaOutput
    }

    pub fn create_audioinput_stream(&self) -> Option<DummyMediaStream> {
        Some(DummyMediaStream)
    }

    pub fn create_videoinput_stream(&self) -> Option<DummyMediaStream> {
        Some(DummyMediaStream)
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

    pub fn create_webrtc(&self, signaller: Box<WebRtcSignaller>) -> WebRtcController {
        WebRtcController::new::<Backend>(signaller)
    }

    pub fn create_audiostream(&self) -> Box<MediaStream> {
        Box::new(Backend::create_audiostream())
    }

    pub fn create_videostream(&self) -> Box<MediaStream> {
        Box::new(Backend::create_videostream())
    }

    pub fn create_stream_output(&self) -> Box<MediaOutput> {
        Box::new(Backend::create_stream_output())
    }

    pub fn create_audioinput_stream(&self) -> Option<Box<MediaStream>> {
        Backend::create_audioinput_stream().map(|s| Box::new(s) as Box<MediaStream>)
    }

    pub fn create_videoinput_stream(&self) -> Option<Box<MediaStream>> {
        Backend::create_videoinput_stream().map(|s| Box::new(s) as Box<MediaStream>)
    }
}
