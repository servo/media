extern crate glib;
extern crate gst_plugin;
extern crate gstreamer_audio as gst_audio;
extern crate gstreamer_base as gst_base;
extern crate gstreamer_player as gst_player;

// XXX not needed at some point.
extern crate byte_slice_cast;
extern crate num_traits;

pub mod src_element;
mod audio_stream;

use AudioStream;
use gst;
use self::audio_stream::GStreamerAudioStream;
use self::src_element::register as register_src_element;
use ServoMediaBackend;

pub struct GStreamer {}

impl GStreamer {
    pub fn new() -> Self {
        gst::init().unwrap();
        register_src_element();
        Self {}
    }
}

impl ServoMediaBackend for GStreamer {
    fn version(&self) -> String {
        gst::version_string()
    }

    fn get_audio_stream(&self) -> Result<Box<AudioStream>, ()> {
        let stream = GStreamerAudioStream::new()?;
        Ok(Box::new(stream))
    }
}
