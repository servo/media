extern crate glib;
extern crate gst_plugin;
extern crate gstreamer_audio as gst_audio;
extern crate gstreamer_base as gst_base;

mod src_element;

use gst;

pub struct GStreamer {}

use ServoMediaBackend;

impl ServoMediaBackend for GStreamer {
    fn backend_id() -> String {
        gst::init().unwrap();
        gst::version_string()
    }
}
