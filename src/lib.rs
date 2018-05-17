use std::sync::{self, Once};
use std::sync::{Arc, Mutex};

#[cfg(feature = "gst")]
extern crate gstreamer as gst;

mod audio;
mod backends;

pub use audio::graph::AudioGraph;
use backends::ServoMediaBackend;

#[cfg(feature = "gst")]
use backends::gstreamer::GStreamer;

pub struct ServoMedia {
    backend: Box<ServoMediaBackend>,
}

static INITIALIZER: Once = sync::ONCE_INIT;
static mut INSTANCE: *mut Mutex<Option<Arc<ServoMedia>>> = 0 as *mut _;

impl ServoMedia {
    pub fn new() -> Self {
        #[cfg(feature = "gst")]
        Self {
            backend: Box::new(GStreamer::new()),
        }
    }

    pub fn get() -> Result<Arc<ServoMedia>, ()> {
        INITIALIZER.call_once(|| unsafe {
            INSTANCE = Box::into_raw(Box::new(Mutex::new(Some(Arc::new(ServoMedia::new())))));
        });
        let instance = unsafe { &*INSTANCE }.lock().unwrap();
        match *instance {
            Some(ref instance) => Ok(instance.clone()),
            None => Err(()),
        }
    }

    pub fn version(&self) -> String {
        self.backend.version()
    }

    pub fn create_audio_graph(&self) -> Result<AudioGraph, ()> {
        AudioGraph::new()
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time};
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
    fn test_audio_graph() {
        if let Ok(servo_media) = ServoMedia::get() {
            let mut graph = servo_media.create_audio_graph().unwrap();
            graph.resume_processing();
            thread::sleep(time::Duration::from_millis(5000));
            graph.pause_processing();
        } else {
            unreachable!();
        }
    }
}
