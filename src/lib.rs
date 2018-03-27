#[cfg(feature = "gst")]
#[macro_use]
extern crate gstreamer as gst;

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

#[cfg(test)]
mod tests {
    #[test]
    fn test_backend_id() {
        use ServoMedia;

        #[cfg(feature = "gst")]
        assert_eq!(ServoMedia::get().version(), "GStreamer 1.14.0");
    }
}
