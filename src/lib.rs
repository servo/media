use std::sync::{self, Once};
use std::sync::{Arc, Mutex};

#[cfg(feature = "gst")]
#[macro_use]
extern crate gstreamer as gst;

mod backends;

#[cfg(feature = "gst")]
use backends::gstreamer::{src_element, GStreamer};

pub trait AudioStream {
    fn play(&self);
    fn stop(&self);
}

pub trait ServoMediaBackend {
    fn version(&self) -> String;
    fn get_audio_stream(&self) -> Result<Box<AudioStream>, ()>;
}

pub struct ServoMedia {}

static INITIALIZER: Once = sync::ONCE_INIT;
static mut INSTANCE: *mut Mutex<Option<Arc<ServoMediaBackend>>> = 0 as *mut _;

impl ServoMedia {
    pub fn get() -> Result<Arc<ServoMediaBackend>, ()> {
        #[cfg(feature = "gst")]
        INITIALIZER.call_once(|| unsafe {
            INSTANCE = Box::into_raw(Box::new(Mutex::new(Some(Arc::new(GStreamer::new())))));
        });
        let instance = unsafe { &*INSTANCE }.lock().unwrap();
        match *instance {
            Some(ref instance) => Ok(instance.clone()),
            None => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use ServoMedia;

    #[test]
    #[cfg(feature = "gst")]
    fn test_backend_id() {
        let servo_media = ServoMedia::get();
        match servo_media {
            Ok(servo_media) => assert_eq!(servo_media.version(), "GStreamer 1.14.0"),
            Err(_) => unreachable!(),
        };
    }

    #[test]
    #[cfg(feature = "gst")]
    fn test_audio_stream() {
        let servo_media = ServoMedia::get();
        match servo_media {
            Ok(servo_media) => servo_media.get_audio_stream().unwrap().play(),
            Err(_) => unreachable!(),
        };
    }
}
