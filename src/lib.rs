#[cfg(feature = "gst")]
#[macro_use]
extern crate gstreamer as gst;

#[cfg(feature = "gst")]
#[macro_use]
extern crate gst_plugin;

mod backends;

#[cfg(feature = "gst")]
use backends::gstreamer::GStreamer;
use backends::gstreamer::src_element;

#[cfg(feature = "gst")]
plugin_define!(
    b"servoaudiosrc\0",
    b"Servo Audio Source\0",
    plugin_init,
    b"1.0\0",
    b"MPL\0",
    b"servoaudiosrc\0",
    b"servoaudiosrc\0",
    b"https://github.com/ferjm/media\0",
    b"2018-03-23\0"
    );

#[cfg(feature = "gst")]
fn plugin_init(plugin: &gst::Plugin) -> bool {
    src_element::register(plugin);
    true
}

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
