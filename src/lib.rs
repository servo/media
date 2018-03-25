#[cfg(feature = "gst")]
#[macro_use]
extern crate gstreamer as gst;

#[cfg(feature = "gst")]
#[macro_use]
extern crate gst_plugin;

mod backends;

#[cfg(feature = "gst")]
use backends::gstreamer::{GStreamer, src_element};

pub trait AudioStream {
    fn play(&self);
    fn stop(&self);
}

pub trait ServoMediaBackend {
    fn version(&self) -> String;
    fn get_audio_stream(&self) -> Result<Box<AudioStream>, ()>;
}

pub struct ServoMedia {}

impl ServoMedia {
    #[cfg(feature = "gst")]
    pub fn get() -> Box<ServoMediaBackend> {
        Box::new(GStreamer::new())
    }
}

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

#[cfg(test)]
mod tests {
    #[test]
    fn test_backend_id() {
        use ServoMedia;

        #[cfg(feature = "gst")]
        assert_eq!(ServoMedia::get().version(), "GStreamer 1.14.0");
    }
}
