use std::sync::{self, Once};
use std::sync::{Arc, Mutex};

#[cfg(feature = "gst")]
extern crate gstreamer as gst;

extern crate byte_slice_cast;
extern crate num_traits;
extern crate petgraph;
extern crate smallvec;

#[macro_use]
pub mod audio;
mod backends;

use audio::graph::AudioGraph;

pub struct ServoMedia {}

static INITIALIZER: Once = sync::ONCE_INIT;
static mut INSTANCE: *mut Mutex<Option<Arc<ServoMedia>>> = 0 as *mut _;

impl ServoMedia {
    pub fn new() -> Self {
        #[cfg(feature = "gst")]
        gst::init().unwrap();

        Self {}
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

    pub fn create_audio_graph(&self) -> AudioGraph {
        AudioGraph::new()
    }
}
