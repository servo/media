pub extern crate servo_media_audio as audio;
#[cfg(any(
    all(target_os = "android", target_arch = "arm"),
    target_arch = "x86_64"
))]
extern crate servo_media_gstreamer;
pub extern crate servo_media_player as player;
use std::sync::{self, Arc, Mutex, Once};

use audio::context::{AudioContext, AudioContextOptions};
use audio::decoder::DummyAudioDecoder;
use audio::sink::{AudioSinkError, DummyAudioSink};
use audio::AudioBackend;
use player::{DummyPlayer, Player, PlayerBackend};

pub struct ServoMedia;

static INITIALIZER: Once = sync::ONCE_INIT;
static mut INSTANCE: *mut Mutex<Option<Arc<ServoMedia>>> = 0 as *mut _;

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
}
