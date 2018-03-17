mod backends;

#[cfg(feature = "gst")]
use backends::gstreamer::GStreamer;

pub trait ServoMediaBackend {
    fn backend_id() -> String;
}

pub struct ServoMedia {}

impl ServoMedia {
    #[cfg(feature = "gst")]
    pub fn backend_id() -> String {
        GStreamer::backend_id()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_backend_id() {
        use ServoMedia;

        #[cfg(feature = "gst")]
        assert_eq!(ServoMedia::backend_id(), "GStreamer 1.12.4");
    }
}
