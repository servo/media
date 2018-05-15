#[cfg(feature = "gst")]
pub mod gstreamer;

pub trait ServoMediaBackend {
    fn version(&self) -> String;
}
