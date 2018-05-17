extern crate glib;
extern crate gst_plugin;
extern crate gstreamer_app as gst_app;
extern crate gstreamer_audio as gst_audio;
extern crate gstreamer_base as gst_base;

pub mod audio_sink;

use gst;
use ServoMediaBackend;

pub struct GStreamer {}

impl GStreamer {
    pub fn new() -> Self {
        gst::init().unwrap();
        Self {}
    }
}

impl ServoMediaBackend for GStreamer {
    fn version(&self) -> String {
        gst::version_string()
    }
}
