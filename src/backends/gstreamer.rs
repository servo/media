extern crate gstreamer as gst;

pub struct GStreamer {}

use ServoMediaBackend;

impl ServoMediaBackend for GStreamer {
    fn backend_id() -> String {
        gst::init().unwrap();
        gst::version_string()
    }
}
