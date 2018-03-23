extern crate glib;
extern crate gstreamer_audio as gst_audio;
extern crate gstreamer_base as gst_base;

// XXX not needed at some point.
extern crate byte_slice_cast;
extern crate num_traits;

pub mod src_element;

use gst;
use ServoMediaBackend;

pub struct GStreamer {}

impl ServoMediaBackend for GStreamer {
    fn backend_id() -> String {
        gst::init().unwrap();
        gst::version_string()
    }
}
