pub extern crate servo_media_audio as audio;
extern crate servo_media_gstreamer;
use servo_media_gstreamer::GStreamerBackend;
use std::sync::{self, Once};
use std::sync::{Arc, Mutex};


use audio::AudioBackend;
use audio::context::{AudioContext, AudioContextOptions};

pub struct ServoMedia {}

static INITIALIZER: Once = sync::ONCE_INIT;
static mut INSTANCE: *mut Mutex<Option<Arc<ServoMedia>>> = 0 as *mut _;

impl ServoMedia {
    pub fn new() -> Self {
        GStreamerBackend::init();

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

    pub fn create_audio_context(&self, options: AudioContextOptions) -> AudioContext<GStreamerBackend> {
        AudioContext::new(options)
    }
}
