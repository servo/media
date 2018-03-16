#[cfg(feature = "gstreamer")]
pub struct GStreamer {}

use ServoMediaBackend;

impl ServoMediaBackend for GStreamer {
    fn backend_id() -> String {
        "GStreamer".to_owned()
    }
}
